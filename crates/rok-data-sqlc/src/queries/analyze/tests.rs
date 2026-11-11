use super::*;
use crate::queries::annotations::parse_query_file;

const SCHEMA: &str = "
        CREATE TABLE categories (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL);
        CREATE TABLE products (
            id UUID PRIMARY KEY,
            sku TEXT NOT NULL,
            name TEXT NOT NULL,
            category_id BIGINT NOT NULL REFERENCES categories (id),
            note TEXT,
            stock INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE app.tags (id BIGSERIAL PRIMARY KEY, \"select\" TEXT NOT NULL);
        CREATE TABLE spans (id BIGSERIAL PRIMARY KEY, length POINT NOT NULL);
        CREATE TYPE product_status AS ENUM ('active', 'retired');
        CREATE TYPE app.ticket_state AS ENUM ('open', 'closed');
        CREATE TABLE gauges (id BIGSERIAL PRIMARY KEY, level TEXT CHECK (level IN ('low', 'high')));
        CREATE TABLE nodes (id BIGSERIAL PRIMARY KEY, parent_id BIGINT REFERENCES nodes (id), label TEXT NOT NULL);
        CREATE TABLE members (id BIGSERIAL PRIMARY KEY, email TEXT NOT NULL, deleted_at TIMESTAMPTZ);
        CREATE UNIQUE INDEX members_email_live ON members (email) WHERE deleted_at IS NULL;
        CREATE TABLE handles (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL);
        CREATE UNIQUE INDEX handles_name_ci ON handles (lower(name));
        CREATE TABLE passes (
            id BIGSERIAL PRIMARY KEY,
            expires_at TIMESTAMPTZ NOT NULL,
            revoked_at TIMESTAMPTZ,
            price NUMERIC NOT NULL,
            discount NUMERIC,
            seats SMALLINT NOT NULL,
            label TEXT NOT NULL
        );
        CREATE FUNCTION tz_today() RETURNS date LANGUAGE sql STABLE
            AS $$ SELECT (now() AT TIME ZONE 'Africa/Nairobi')::date $$;
        CREATE FUNCTION app.seats_left(pass BIGINT, taken INTEGER) RETURNS integer
            LANGUAGE sql AS $$ SELECT 4 - taken $$;
        CREATE FUNCTION touch() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            RETURN NEW;
        END;
        $$;
        CREATE TABLE partners (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL, active BOOLEAN NOT NULL);
        CREATE TABLE offers (id BIGSERIAL PRIMARY KEY, partner_id BIGINT NOT NULL REFERENCES partners (id), status TEXT NOT NULL);
        CREATE TABLE allocations (
            id BIGSERIAL PRIMARY KEY,
            offer_id BIGINT NOT NULL REFERENCES offers (id),
            rooms_held INTEGER NOT NULL,
            rooms_total INTEGER NOT NULL,
            held_at TIMESTAMPTZ
        );
        CREATE TABLE media (id UUID PRIMARY KEY, url TEXT NOT NULL);
        CREATE TABLE partner_photos (
            id UUID PRIMARY KEY,
            partner_id BIGINT NOT NULL REFERENCES partners (id),
            media_id UUID NOT NULL REFERENCES media (id),
            caption TEXT,
            sort INTEGER NOT NULL
        );
        CREATE TABLE listings (
            id BIGSERIAL PRIMARY KEY,
            partner_id BIGINT NOT NULL REFERENCES partners (id),
            cover_media_id UUID REFERENCES media (id),
            status product_status NOT NULL
        );
        CREATE TABLE weekly_holds (
            id BIGSERIAL PRIMARY KEY,
            offer_id BIGINT NOT NULL REFERENCES offers (id),
            week_of DATE NOT NULL,
            rooms INTEGER NOT NULL
        );
        CREATE TABLE order_items (
            id BIGSERIAL PRIMARY KEY,
            order_id UUID NOT NULL,
            item_id UUID NOT NULL,
            quantity INTEGER NOT NULL,
            note TEXT
        );
    ";

fn analyze(queries: &str) -> (Vec<AnalyzedQuery>, Vec<QueryFailure>) {
    let parsed = crate::parser::parse_schema(SCHEMA).unwrap();
    analyze_against(queries, &parsed, &parsed.schema.tables)
}

fn analyze_against(
    queries: &str,
    parsed: &crate::parser::ParsedSchema,
    tables: &[Table],
) -> (Vec<AnalyzedQuery>, Vec<QueryFailure>) {
    let (defs, failures) = parse_query_file(queries);
    assert!(failures.is_empty(), "{failures:?}");
    let modules = tables
        .iter()
        .map(|table| table.module_name.clone())
        .collect();
    analyze_queries(
        &defs,
        tables,
        &parsed.schema.enums,
        &parsed.schema.functions,
        &modules,
        &parsed.failures,
    )
}

/// The schema's tables with their typed ids, as `rok gen` analyzes queries
/// against them.
fn typed_tables() -> (crate::parser::ParsedSchema, Vec<Table>) {
    let parsed = crate::parser::parse_schema(SCHEMA).unwrap();
    let tables = crate::generator::keys::resolve(&parsed.schema.tables);
    (parsed, tables)
}

/// [`one`], with typed ids.
fn typed_one(queries: &str) -> AnalyzedQuery {
    let (parsed, tables) = typed_tables();
    let (mut analyzed, failures) = analyze_against(queries, &parsed, &tables);
    assert!(failures.is_empty(), "{failures:?}");
    analyzed.remove(0)
}

fn one(queries: &str) -> AnalyzedQuery {
    let (mut analyzed, failures) = analyze(queries);
    assert!(failures.is_empty(), "{failures:?}");
    analyzed.remove(0)
}

fn failure(queries: &str) -> String {
    let (analyzed, failures) = analyze(queries);
    assert!(analyzed.is_empty(), "expected a failure, got {analyzed:?}");
    failures[0].reason.clone()
}

fn param(index: usize, name: &str, rust_type: &str) -> Param {
    Param {
        index,
        name: name.to_owned(),
        rust_type: rust_type.to_owned(),
    }
}

#[test]
fn infers_params_from_comparisons_and_limits() {
    let query = one(
        "-- name: ListProducts :many\nSELECT id, sku FROM products WHERE category_id = $1 AND stock <= $2 ORDER BY sku LIMIT $3 OFFSET $4;",
    );
    assert_eq!(query.fn_name, "list_products");
    assert_eq!(
        query.params,
        [
            param(1, "category_id", "i64"),
            param(2, "stock", "i32"),
            param(3, "limit", "i64"),
            param(4, "offset", "i64"),
        ]
    );
    assert_eq!(
        query.sql,
        "SELECT id, sku FROM products WHERE category_id = $1 AND stock <= $2 ORDER BY sku LIMIT $3 OFFSET $4"
    );
    let ResultShape::Row(columns) = &query.result else {
        panic!("expected a row struct, got {:?}", query.result);
    };
    assert_eq!(columns[0].rust_type, "uuid::Uuid");
    assert_eq!(columns[1].rust_name, "sku");
}

/// A `:page` query is written once, with its `WHERE` and `ORDER BY`: its
/// parameters and rows are the query's own, the page is the function's.
#[test]
fn a_page_query_is_analyzed_like_a_many() {
    let query = one(
        "-- name: ProductsPage :page\nSELECT id, sku FROM products WHERE category_id = $1 AND (sku ILIKE $2 OR name ILIKE $2) ORDER BY sku;",
    );
    assert_eq!(query.mode, ReturnMode::Page);
    assert_eq!(
        query.params,
        [param(1, "category_id", "i64"), param(2, "sku", "String")]
    );
    assert!(matches!(query.result, ResultShape::Row(_)), "{query:?}");
}

/// A page holds rows, so even one column is a `{Name}Row`.
#[test]
fn a_one_column_page_is_a_row() {
    let query = one("-- name: Skus :page\nSELECT sku FROM products ORDER BY sku;");
    let ResultShape::Row(columns) = &query.result else {
        panic!("expected a row struct, got {:?}", query.result);
    };
    assert_eq!(columns[0].rust_name, "sku");
}

#[test]
fn a_page_query_with_its_own_limit_or_offset_fails_naming_it() {
    let expected = |clause: &str| {
        format!(
            "a `:page` query takes no `{clause}`: the generated function adds `LIMIT`/`OFFSET` from its `page` and `per_page`"
        )
    };
    assert_eq!(
        failure("-- name: P :page\nSELECT id FROM products ORDER BY sku LIMIT 10;"),
        expected("LIMIT")
    );
    assert_eq!(
        failure("-- name: P :page\nSELECT id FROM products ORDER BY sku OFFSET $1;"),
        expected("OFFSET")
    );
    assert_eq!(
        failure("-- name: P :page\nSELECT id FROM products ORDER BY sku FETCH FIRST 5 ROWS ONLY;"),
        expected("FETCH")
    );
}

#[test]
fn a_page_query_needs_an_order_by() {
    assert_eq!(
        failure("-- name: P :page\nSELECT id FROM products WHERE stock > 0;"),
        "a `:page` query needs an `ORDER BY`: without one its pages hold rows in no particular order, so a row can show on two pages or none"
    );
}

#[test]
fn a_page_query_is_a_select() {
    assert_eq!(
        failure("-- name: P :page\nDELETE FROM products WHERE stock = 0 RETURNING id;"),
        "a `:page` query is a `SELECT`; this one is a `DELETE`"
    );
}

#[test]
fn casts_to_interval_bytea_and_timetz_infer_their_mapped_types() {
    let query = one(
        "-- name: Q :many\nSELECT id, '1 day'::interval AS span, sku::bytea AS raw, '12:00+01'::timetz AS opens \
             FROM products WHERE $1::interval IS NOT NULL AND $2::bytea IS NOT NULL AND $3::time with time zone IS NOT NULL;",
    );
    let interval = "sqlx::postgres::types::PgInterval";
    let time_tz = "sqlx::postgres::types::PgTimeTz";
    assert_eq!(
        query.params,
        [
            param(1, "param_1", interval),
            param(2, "param_2", "Vec<u8>"),
            param(3, "param_3", time_tz),
        ]
    );
    let ResultShape::Row(columns) = &query.result else {
        panic!("expected a row struct");
    };
    let types: Vec<&str> = columns
        .iter()
        .map(|column| column.rust_type.as_str())
        .collect();
    assert_eq!(
        types[1..],
        [
            // A cast keeps the nullability of what it wraps: a literal is
            // never NULL, and `sku` is `NOT NULL`.
            interval.to_owned(),
            "Vec<u8>".to_owned(),
            time_tz.to_owned(),
        ]
    );
}

#[test]
fn infers_params_from_in_any_between_like_and_casts() {
    let query = one(
        "-- name: Search :many\nSELECT id, sku FROM products WHERE sku LIKE $1 AND stock BETWEEN $2 AND $3 AND category_id IN ($4, $5) AND id = ANY($6) AND name = $7::text;",
    );
    assert_eq!(
        query.params,
        [
            param(1, "sku", "String"),
            param(2, "stock_from", "i32"),
            param(3, "stock_to", "i32"),
            param(4, "category_id", "i64"),
            param(5, "category_id_2", "i64"),
            param(6, "id_list", "Vec<uuid::Uuid>"),
            param(7, "name", "String"),
        ]
    );
}

#[test]
fn insert_values_take_column_types_and_nullability() {
    let query = one(
        "-- name: CreateProducts :execrows\nINSERT INTO products (id, sku, name, category_id, note) VALUES ($1, $2, $3, $4, $5), ($6, $7, $8, $9, NULL);",
    );
    assert_eq!(query.params[4], param(5, "note", "Option<String>"));
    assert_eq!(query.params[5], param(6, "id_2", "uuid::Uuid"));
    assert_eq!(query.params.len(), 9);
    assert_eq!(query.result, ResultShape::None);
}

#[test]
fn update_set_and_upsert_assignments_are_typed() {
    let query =
        one("-- name: Rename :exec\nUPDATE products SET name = $2, note = $3 WHERE id = $1;");
    assert_eq!(
        query.params,
        [
            param(1, "id", "uuid::Uuid"),
            param(2, "name", "String"),
            param(3, "note", "Option<String>"),
        ]
    );
    let upsert = one(
        "-- name: Upsert :exec\nINSERT INTO products (id, sku, name, category_id) VALUES ($1, $2, $3, $4)\nON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, stock = $5;",
    );
    assert_eq!(upsert.params[4], param(5, "stock", "i32"));
}

#[test]
fn between_bounds_are_named_from_and_to() {
    let query = one(
        "-- name: InStock :many\nSELECT id FROM products WHERE stock BETWEEN $1 AND $2 AND stock NOT BETWEEN $3 AND $4::int;",
    );
    assert_eq!(
        query.params,
        [
            param(1, "stock_from", "i32"),
            param(2, "stock_to", "i32"),
            param(3, "stock_from_2", "i32"),
            param(4, "stock_to_2", "i32"),
        ]
    );
}

#[test]
fn a_param_override_without_a_type_renames_and_keeps_the_inferred_type() {
    let query = one(
        "-- name: InStock :many\n-- param: $1 from\n-- param: $2 to\nSELECT id FROM products WHERE category_id = $3 AND stock BETWEEN $1 AND $2;",
    );
    assert_eq!(
        query.params,
        [
            param(1, "from", "i32"),
            param(2, "to", "i32"),
            param(3, "category_id", "i64"),
        ]
    );
}

#[test]
fn a_bang_marks_a_nullable_columns_param_not_null() {
    let query = one(
        "-- name: Annotate :exec\n-- param: $2 note!\n-- param: $3 memo!\nUPDATE products SET note = $2 WHERE id = $1 AND note <> $3;",
    );
    assert_eq!(
        query.params,
        [
            param(1, "id", "uuid::Uuid"),
            param(2, "note", "String"),
            param(3, "memo", "String"),
        ]
    );
    let typed_id = one(
        "-- name: Reparent :exec\n-- param: $1 parent!\nUPDATE nodes SET parent_id = $1 WHERE id = $2;",
    );
    assert_eq!(typed_id.params[0].name, "parent");
    assert!(
        !typed_id.params[0].rust_type.starts_with("Option<"),
        "{:?}",
        typed_id.params
    );
}

#[test]
fn a_type_free_override_needs_an_inferable_placeholder() {
    let reason = failure(
        "-- name: Q :many\n-- param: $1 pattern\nSELECT id FROM products WHERE lower(name) = lower($1);",
    );
    assert!(
        reason.contains("`-- param: $1 pattern <RustType>`"),
        "{reason}"
    );
}

/// Karibu Pass's `HoldRooms`: an `UPDATE … FROM` with a join, typed like
/// the equivalent join.
const HOLD_ROOMS: &str = "UPDATE allocations SET rooms_held = rooms_held + $2, held_at = $3
FROM offers o JOIN partners p ON p.id = o.partner_id
WHERE o.id = allocations.offer_id AND allocations.id = $1 AND p.active AND o.status = $4
  AND rooms_held + $2 <= rooms_total";

#[test]
fn update_from_resolves_columns_across_the_target_and_its_from_tables() {
    let exec = one(&format!("-- name: HoldRooms :execrows\n{HOLD_ROOMS};"));
    assert_eq!(
        exec.params,
        [
            param(1, "id", "i64"),
            param(2, "rooms_held", "i32"),
            param(3, "held_at", "Option<chrono::DateTime<chrono::Utc>>"),
            param(4, "status", "String"),
        ]
    );
    assert_eq!(exec.result, ResultShape::None);

    let returning = one(&format!(
        "-- name: HoldRooms :maybe_one\n{HOLD_ROOMS}\nRETURNING allocations.id, rooms_held, o.status, p.name AS partner;"
    ));
    assert_eq!(returning.params, exec.params);
    assert_eq!(
        row_columns(&returning),
        [
            ("id", "i64"),
            ("rooms_held", "i32"),
            ("status", "String"),
            ("partner", "String"),
        ]
    );
}

/// Karibu Pass's `AddPhoto`: an `INSERT … SELECT` whose values fill the
/// target's columns, typed from them as `VALUES` is, while its `FROM` and
/// `WHERE` see only the `SELECT`'s own tables.
const ADD_PHOTO: &str = "INSERT INTO partner_photos (id, partner_id, media_id, caption, sort)
SELECT $1, $2, $3, $4, coalesce(max(sort) + 1, 0)
FROM partner_photos WHERE partner_id = $2
ON CONFLICT DO NOTHING";

#[test]
fn insert_select_types_its_values_from_the_target_columns() {
    let exec = one(&format!("-- name: AddPhoto :exec\n{ADD_PHOTO};"));
    assert_eq!(
        exec.params,
        [
            param(1, "id", "uuid::Uuid"),
            param(2, "partner_id", "i64"),
            param(3, "media_id", "uuid::Uuid"),
            param(4, "caption", "Option<String>"),
        ]
    );
    assert_eq!(exec.result, ResultShape::None);

    let returning = one(&format!(
        "-- name: AddPhoto :maybe_one\n{ADD_PHOTO}\nRETURNING id, sort;"
    ));
    assert_eq!(returning.params, exec.params);
    assert_eq!(
        row_columns(&returning),
        [("id", "uuid::Uuid"), ("sort", "i32")]
    );
}

#[test]
fn insert_select_upserts_like_values() {
    let query = one(
        "-- name: CopyStock :exec\nINSERT INTO products (id, sku, name, category_id, stock)\nSELECT $1, sku, name, category_id, stock FROM products WHERE id = $2\nON CONFLICT (id) DO UPDATE SET stock = EXCLUDED.stock + $3;",
    );
    assert_eq!(
        query.params,
        [
            param(1, "id", "uuid::Uuid"),
            param(2, "id_2", "uuid::Uuid"),
            param(3, "stock", "i32"),
        ]
    );
}

#[test]
fn insert_select_with_the_wrong_number_of_values_fails_naming_them() {
    let reason = failure(
        "-- name: Q :exec\nINSERT INTO partner_photos (id, partner_id, media_id) SELECT $1, $2 FROM partners WHERE id = $2;",
    );
    assert!(
        reason.contains("the SELECT returns 2 values for 3 columns"),
        "{reason}"
    );
}

#[test]
fn update_from_names_the_tables_of_an_ambiguous_column() {
    let reason = failure(
        "-- name: Q :exec\nUPDATE allocations SET rooms_held = 0 FROM offers o WHERE id = $1;",
    );
    assert!(reason.contains("column `id` is ambiguous"), "{reason}");
    assert!(reason.contains("`allocations.id`"), "{reason}");
    assert!(reason.contains("`o.id`"), "{reason}");
}

#[test]
fn delete_using_resolves_columns_like_a_join() {
    let query = one(
        "-- name: DropRetired :many\nDELETE FROM allocations a USING offers o, partners p\nWHERE o.id = a.offer_id AND p.id = o.partner_id AND p.name = $1 AND o.status = $2\nRETURNING a.id, o.status;",
    );
    assert_eq!(
        query.params,
        [param(1, "name", "String"), param(2, "status", "String")]
    );
    assert_eq!(row_columns(&query), [("id", "i64"), ("status", "String")]);
}

#[test]
fn a_rok_skip_query_is_neither_analyzed_nor_failed() {
    let (analyzed, failures) = analyze(
        "-- name: Untypeable :one\n-- rok:skip\nSELECT upper(name) FROM products;\n-- name: Kept :exec\nDELETE FROM products;",
    );
    assert!(failures.is_empty(), "{failures:?}");
    let names: Vec<&str> = analyzed.iter().map(|query| query.name.as_str()).collect();
    assert_eq!(names, ["Kept"]);
}

#[test]
fn arithmetic_with_a_numeric_column_types_its_placeholder() {
    let query = one(
        "-- name: Q :exec\nUPDATE allocations SET rooms_held = rooms_total - $1 WHERE id = $2;",
    );
    assert_eq!(query.params[0], param(1, "rooms_total", "i32"));
    // A timestamp plus a placeholder is a timestamp plus an interval: not
    // settled by the column.
    assert!(
        failure("-- name: Q :exec\nUPDATE allocations SET held_at = held_at + $1;")
            .contains("cannot infer the type of `$1`")
    );
}

#[test]
fn returning_star_reuses_the_resource_model() {
    let query =
        one("-- name: CreateCategory :one\nINSERT INTO categories (name) VALUES ($1) RETURNING *;");
    assert_eq!(
        query.result,
        ResultShape::Model {
            module: "categories".to_owned(),
            struct_name: "Category".to_owned()
        }
    );
}

#[test]
fn casts_to_enum_types_infer_the_generated_enum() {
    let query = one(
        "-- name: Statuses :many\nSELECT id FROM products WHERE $1::product_status IS NOT NULL AND $2::app.ticket_state IS NOT NULL AND $3::product_status[] IS NOT NULL AND $4::public.product_status IS NOT NULL;",
    );
    assert_eq!(
        query.params,
        [
            param(1, "param_1", "types::ProductStatus"),
            param(2, "param_2", "types::TicketState"),
            param(3, "param_3", "Vec<types::ProductStatus>"),
            param(4, "param_4", "types::ProductStatus"),
        ]
    );
    let status =
        one("-- name: Status :one\nSELECT 'active'::product_status AS status FROM products;");
    assert_eq!(
        status.result,
        // A cast literal is never NULL.
        ResultShape::Scalar("types::ProductStatus".to_owned())
    );
}

/// An enum type is found under its schema, and a value-list enum is no
/// database type at all.
#[test]
fn casts_to_other_names_are_not_enums() {
    for cast in ["ticket_state", "app.product_status", "level"] {
        let reason = failure(&format!(
            "-- name: Q :many\nSELECT id FROM gauges WHERE $1::{cast} IS NOT NULL;"
        ));
        assert!(reason.contains("is not supported yet"), "{cast}: {reason}");
    }
}

#[test]
fn partial_selects_get_row_structs_and_single_columns_scalars() {
    let row = one("-- name: Names :many\nSELECT name, id FROM categories;");
    assert!(matches!(row.result, ResultShape::Row(_)));
    let scalar = one("-- name: CountProducts :one\nSELECT COUNT(*) FROM products;");
    assert_eq!(scalar.result, ResultShape::Scalar("i64".to_owned()));
    let note = one("-- name: Note :maybe_one\nSELECT note FROM products WHERE id = $1;");
    assert_eq!(
        note.result,
        ResultShape::Scalar("Option<String>".to_owned())
    );
}

#[test]
fn left_joins_make_the_right_side_nullable() {
    let query = one(
        "-- name: WithCategory :many\nSELECT p.id, p.sku, c.name AS category_name FROM products p LEFT JOIN categories c ON c.id = p.category_id WHERE c.name = $1;",
    );
    let ResultShape::Row(columns) = &query.result else {
        panic!("expected a row struct");
    };
    assert_eq!(columns[2].sql_name, "category_name");
    assert_eq!(columns[2].rust_type, "Option<String>");
    assert_eq!(query.params, [param(1, "name", "String")]);
}

#[test]
fn schema_qualified_and_quoted_names_resolve() {
    let query =
        one("-- name: Tags :many\nSELECT id, \"select\" FROM app.tags WHERE \"select\" = $1;");
    assert_eq!(
        query.result,
        ResultShape::Model {
            module: "tags".to_owned(),
            struct_name: "Tag".to_owned()
        }
    );
    assert_eq!(query.params, [param(1, "select", "String")]);
}

#[test]
fn overrides_supply_types_inference_cannot() {
    let query = one(
        "-- name: Totals :many\n-- param: $1 min_total i64\n-- column: total i64\nSELECT category_id, SUM(stock)::bigint AS total FROM products GROUP BY category_id HAVING SUM(stock) > $1;",
    );
    assert_eq!(query.params, [param(1, "min_total", "i64")]);
    let ResultShape::Row(columns) = &query.result else {
        panic!("expected a row struct");
    };
    assert_eq!(columns[1].rust_type, "i64");
}

#[test]
fn reports_untypable_and_unused_placeholders() {
    assert!(
        failure("-- name: Q :many\nSELECT id FROM products WHERE lower(name) = lower($1);")
            .contains("add `-- param: $1 <name> <RustType>`")
    );
    assert!(
        failure("-- name: Q :many\nSELECT id FROM products WHERE sku = $2;")
            .contains("`$1` is never used")
    );
    assert!(
        failure("-- name: Q :many\nSELECT id FROM products WHERE sku = $1 OR stock = $1;")
            .contains("used both as `String` and as `i32`")
    );
}

#[test]
fn reports_schema_mismatches() {
    assert!(
        failure("-- name: Q :many\nSELECT id FROM nope;").contains("`nope` is not in the schema")
    );
    assert!(
        failure("-- name: Q :many\nSELECT colour FROM products;")
            .contains("column `colour` does not exist in `products`")
    );
    assert!(
            failure(
                "-- name: Q :many\nSELECT id, name FROM products p JOIN categories c ON c.id = p.category_id;"
            )
            .contains("ambiguous")
        );
    assert!(
        failure("-- name: Q :many\nSELECT id FROM spans;")
            .contains("table `spans` is not generatable")
    );
}

#[test]
fn reports_unsupported_statement_shapes() {
    assert!(
        failure("-- name: Q :exec\nBEGIN;\nDELETE FROM products;\nCOMMIT;")
            .contains("contains 3 statements")
    );
    assert!(
        failure("-- name: Q :one\nDELETE FROM products WHERE id = $1;")
            .contains("add RETURNING or use `:exec`")
    );
    assert!(
        failure("-- name: Q :many\nSELECT upper(name) FROM products;")
            .contains("`-- column: upper <RustType>`")
    );
    assert!(
        failure("-- name: Q :many\nSELECT id, id FROM products;")
            .contains("appears more than once")
    );
    assert!(failure("-- name: Q :many\nSELECT 1 FROM products;").contains("needs an alias"));
}

fn row_columns(query: &AnalyzedQuery) -> Vec<(&str, &str)> {
    let ResultShape::Row(columns) = &query.result else {
        panic!("expected a row struct, got {:?}", query.result);
    };
    columns
        .iter()
        .map(|column| (column.sql_name.as_str(), column.rust_type.as_str()))
        .collect()
}

#[test]
fn cte_results_join_like_tables_and_type_their_placeholders() {
    let query = one("-- name: StockByCategory :many\n\
             WITH stock AS (\n\
                 SELECT category_id, SUM(stock)::bigint AS total FROM products WHERE stock >= $1 GROUP BY category_id\n\
             )\n\
             SELECT c.id, c.name, s.total FROM categories c JOIN stock s ON s.category_id = c.id WHERE s.total > $2;");
    assert_eq!(
        row_columns(&query),
        [("id", "i64"), ("name", "String"), ("total", "Option<i64>")]
    );
    assert_eq!(
        query.params,
        [param(1, "stock", "i32"), param(2, "total", "i64")]
    );
}

#[test]
fn later_ctes_see_earlier_ones_and_column_lists_rename() {
    let query = one("-- name: BigCategories :many\n\
             WITH counted (cat, n) AS (SELECT category_id, COUNT(*) FROM products GROUP BY category_id),\n\
                  big AS (SELECT cat, n AS product_count FROM counted WHERE n > $1)\n\
             SELECT big.cat, big.product_count, c.name FROM big JOIN categories c ON c.id = big.cat;");
    assert_eq!(
        row_columns(&query),
        [("cat", "i64"), ("product_count", "i64"), ("name", "String")]
    );
    assert_eq!(query.params, [param(1, "n", "i64")]);
}

#[test]
fn a_cte_column_list_names_unaliased_expressions() {
    let query = one(
        "-- name: Totals :many\nWITH t (category_id, total) AS (SELECT category_id, SUM(stock)::bigint FROM products GROUP BY category_id) SELECT category_id, total FROM t;",
    );
    assert_eq!(
        row_columns(&query),
        [("category_id", "i64"), ("total", "Option<i64>")]
    );
    assert!(
        failure(
            "-- name: Q :many\nWITH t AS (SELECT SUM(stock)::bigint FROM products) SELECT * FROM t;"
        )
        .contains("needs an alias")
    );
}

#[test]
fn a_cte_selecting_a_whole_table_does_not_reuse_its_model() {
    let query =
        one("-- name: Cats :many\nWITH cats AS (SELECT * FROM categories) SELECT * FROM cats;");
    assert_eq!(row_columns(&query), [("id", "i64"), ("name", "String")]);
}

#[test]
fn ctes_are_scoped_to_their_statement_and_must_be_typable() {
    assert!(
            failure("-- name: Q :many\nWITH a AS (SELECT id FROM b), b AS (SELECT id FROM categories) SELECT id FROM a;")
                .contains("table `b` is not in the schema")
        );
    assert!(
            failure("-- name: Q :many\nWITH a AS (SELECT SUM(stock) AS total FROM products) SELECT total FROM a;")
                .contains("cannot infer the type of column `total` of CTE `a`")
        );
    assert!(
        failure("-- name: Q :many\nWITH a (x, y) AS (SELECT id FROM categories) SELECT x FROM a;")
            .contains("CTE `a` names 2 columns but its query returns 1")
    );
}

/// The anchor types a recursive CTE; the recursive term sees the CTE and
/// can make a column nullable.
#[test]
fn recursive_ctes_take_their_columns_from_the_anchor() {
    let walk = one(
        "-- name: Subtree :many\nWITH RECURSIVE tree (id, parent_id, depth) AS (\n    SELECT id, parent_id, 0::int FROM nodes WHERE id = $1\n    UNION ALL\n    SELECT n.id, n.parent_id, t.depth + 1 FROM nodes n JOIN tree t ON n.parent_id = t.id\n)\nSELECT id, parent_id, depth FROM tree;",
    );
    assert_eq!(
        row_columns(&walk),
        [
            ("id", "i64"),
            ("parent_id", "Option<i64>"),
            // A cast literal is never NULL, and nor is `depth + 1`.
            ("depth", "i32")
        ]
    );
    assert_eq!(walk.params, [param(1, "id", "i64")]);

    let widened = one(
        "-- name: Labels :many\nWITH RECURSIVE t AS (\n    SELECT id, label FROM nodes WHERE parent_id IS NULL\n    UNION\n    SELECT n.id, NULL::text FROM nodes n JOIN t ON n.parent_id = t.id\n)\nSELECT id, label FROM t;",
    );
    assert_eq!(
        row_columns(&widened),
        [("id", "i64"), ("label", "Option<String>")]
    );
}

#[test]
fn recursive_terms_must_match_the_anchor() {
    assert_eq!(
        failure(
            "-- name: Q :many\nWITH RECURSIVE t (n) AS (SELECT 1::int UNION ALL SELECT n, n FROM t) SELECT n FROM t;"
        ),
        "the recursive term of CTE `t` returns 2 columns but its anchor returns 1"
    );
    assert_eq!(
        failure(
            "-- name: Q :many\nWITH RECURSIVE t (n) AS (SELECT 1::int UNION ALL SELECT n::bigint FROM t) SELECT n FROM t;"
        ),
        "the recursive term of CTE `t` returns `i64` for column `n`, but its anchor returns `i32`; cast one of them"
    );
}

/// Karibu Pass's `CancelEvent` shape: two chained `UPDATE … RETURNING`
/// CTEs, the second reading the first, feeding an `INSERT … SELECT`. The
/// final statement decides the result.
const CANCEL_OFFER: &str = "WITH o AS (\n    UPDATE offers SET status = 'cancelled' WHERE id = $1 AND status <> 'cancelled' RETURNING id, partner_id\n),\na AS (\n    UPDATE allocations SET rooms_held = 0 FROM o\n    WHERE allocations.offer_id = o.id AND allocations.rooms_total > $2\n    RETURNING allocations.id, allocations.rooms_held\n)\nINSERT INTO allocations (offer_id, rooms_held, rooms_total)\nSELECT o.id, $3, a.rooms_held FROM o JOIN a ON true";

#[test]
fn data_modifying_ctes_feed_the_final_statement() {
    let rows = one(&format!("-- name: CancelOffer :execrows\n{CANCEL_OFFER};"));
    assert_eq!(rows.result, ResultShape::None);
    assert_eq!(
        rows.params,
        [
            param(1, "id", "i64"),
            param(2, "rooms_total", "i32"),
            param(3, "rooms_held", "i32"),
        ]
    );

    let returning = one(&format!(
        "-- name: CancelOffer :many\n{CANCEL_OFFER}\nRETURNING id, held_at;"
    ));
    assert_eq!(returning.params, rows.params);
    assert_eq!(
        row_columns(&returning),
        [
            ("id", "i64"),
            ("held_at", "Option<chrono::DateTime<chrono::Utc>>")
        ]
    );
}

/// Karibu Pass's `PlaceOrder` shape: an `INSERT … RETURNING` CTE feeding an
/// `UPDATE … FROM`; a `DELETE … RETURNING` CTE read by a `SELECT`.
#[test]
fn any_final_statement_reads_a_data_modifying_cte() {
    let update = one(
        "-- name: PlaceOffer :one\nWITH o AS (INSERT INTO offers (partner_id, status) VALUES ($1, 'new') RETURNING id)\nUPDATE allocations SET offer_id = o.id FROM o WHERE allocations.id = $2 RETURNING allocations.id;",
    );
    assert_eq!(
        update.params,
        [param(1, "partner_id", "i64"), param(2, "id", "i64")]
    );
    assert_eq!(update.result, ResultShape::Scalar("i64".to_owned()));

    let select = one(
        "-- name: Purge :many\nWITH gone AS (DELETE FROM products WHERE stock = $1 RETURNING id, note) SELECT id, note FROM gone;",
    );
    assert_eq!(select.params, [param(1, "stock", "i32")]);
    assert_eq!(
        row_columns(&select),
        [("id", "uuid::Uuid"), ("note", "Option<String>")]
    );
}

/// PostgreSQL runs a data-modifying CTE only in the statement's own `WITH`;
/// a `:page` query would run it twice, once for its rows and once for its
/// total.
#[test]
fn data_modifying_ctes_belong_to_the_top_level_with() {
    assert_eq!(
        failure(
            "-- name: Q :many\nWITH a AS (WITH g AS (DELETE FROM products RETURNING id) SELECT id FROM g) SELECT id FROM a;"
        ),
        "data-modifying CTE `g` must be in the statement's own WITH, not nested in a CTE or subquery"
    );
    assert!(
        failure(
            "-- name: Q :page\nWITH g AS (DELETE FROM products RETURNING id) SELECT id FROM g ORDER BY id;"
        )
        .contains("a `:page` query can't have a data-modifying CTE")
    );
}

/// A partial unique index is a target when the target repeats its
/// condition; the generated function keeps the SQL as written.
#[test]
fn partial_unique_index_targets_need_their_condition() {
    let upsert = one(
        "-- name: UpsertMember :one\nINSERT INTO members (email) VALUES ($1)\nON CONFLICT (email) WHERE (deleted_at IS NULL) DO UPDATE SET email = EXCLUDED.email\nRETURNING id;",
    );
    assert!(
        upsert.sql.contains("WHERE (deleted_at IS NULL) DO UPDATE"),
        "{}",
        upsert.sql
    );
    assert_eq!(upsert.params, [param(1, "email", "String")]);

    // A full key matches with any predicate, as PostgreSQL infers it too.
    one(
        "-- name: ById :exec\nINSERT INTO members (id, email) VALUES ($1, $2)\nON CONFLICT (id) WHERE deleted_at IS NULL DO NOTHING;",
    );

    assert_eq!(
        failure(
            "-- name: Missing :exec\nINSERT INTO members (email) VALUES ($1) ON CONFLICT (email) DO NOTHING;"
        ),
        "ON CONFLICT (email) matches only a partial unique index of `members`, whose condition is `deleted_at IS NULL`; add `WHERE deleted_at IS NULL` to the target"
    );
    assert_eq!(
        failure(
            "-- name: Other :exec\nINSERT INTO members (email) VALUES ($1) ON CONFLICT (email) WHERE deleted_at IS NOT NULL DO NOTHING;"
        ),
        "ON CONFLICT (email) matches only a partial unique index of `members`, whose condition is `deleted_at IS NULL`; the target's `WHERE deleted_at IS NOT NULL` must be `WHERE deleted_at IS NULL`"
    );
    assert!(
        failure("-- name: Bound :exec\nINSERT INTO members (email) VALUES ($1) ON CONFLICT (email) WHERE email <> $2 DO NOTHING;")
            .starts_with("a placeholder in the ON CONFLICT target's WHERE is not supported"),
    );
}

/// Taking the predicate out keeps the rest of the text as written, so
/// escaped quotes and other lines survive analysis.
#[test]
fn partial_index_targets_keep_escaped_quotes() {
    one(
        "-- name: Quoted :exec\nINSERT INTO members (email) VALUES ('it''s')\nON CONFLICT (email)\n    WHERE deleted_at IS NULL\nDO UPDATE SET email = 'O''Brien';",
    );
}

/// PostgreSQL infers a unique index on `lower(name)` only from the target
/// `(lower(name))`, so the column target `(name)` matches no unique key.
#[test]
fn case_insensitive_unique_keys_are_not_column_targets() {
    assert_eq!(
        failure(
            "-- name: Upsert :exec\nINSERT INTO handles (name) VALUES ($1) ON CONFLICT (name) DO NOTHING;"
        ),
        "ON CONFLICT (name) matches no unique key; unique keys of `handles`: (id)"
    );
}

fn scalar(queries: &str) -> String {
    match one(queries).result {
        ResultShape::Scalar(rust_type) => rust_type,
        other => panic!("expected a scalar, got {other:?}"),
    }
}

/// A cast keeps the nullability of what it wraps: `count(*)` is never NULL.
#[test]
fn a_cast_count_is_not_null() {
    assert_eq!(
        scalar("-- name: Owners :one\nSELECT count(*)::bigint AS owners FROM products;"),
        "i64"
    );
}

/// Ticket 31's rule holds with a `FILTER`: a count is never NULL.
#[test]
fn a_cast_filtered_count_is_not_null() {
    assert_eq!(
        scalar(
            "-- name: Restocked :one\nSELECT count(*) FILTER (WHERE stock > 0)::bigint AS restocked FROM products;"
        ),
        "i64"
    );
}

#[test]
fn a_cast_not_null_column_is_not_null() {
    assert_eq!(
        scalar("-- name: Stock :one\nSELECT stock::bigint AS stock FROM products WHERE id = $1;"),
        "i64"
    );
}

#[test]
fn a_cast_nullable_column_stays_nullable() {
    assert_eq!(
        scalar("-- name: Note :one\nSELECT note::varchar AS note FROM products WHERE id = $1;"),
        "Option<String>"
    );
}

/// The outer side of a join can be NULL even where the column is `NOT NULL`.
#[test]
fn a_cast_outer_join_column_stays_nullable() {
    assert_eq!(
        scalar(
            "-- name: Stock :one\nSELECT p.stock::bigint AS stock FROM categories c LEFT JOIN products p ON p.category_id = c.id WHERE c.id = $1;"
        ),
        "Option<i64>"
    );
}

/// `coalesce` whose last argument is a non-NULL literal is never NULL.
#[test]
fn coalesce_with_a_literal_fallback_is_not_null() {
    assert_eq!(
        scalar(
            "-- name: Note :one\nSELECT coalesce(note, '') AS note FROM products WHERE id = $1;"
        ),
        "String"
    );
    assert_eq!(
        scalar(
            "-- name: Stock :one\nSELECT coalesce(p.stock, 0) AS stock FROM categories c LEFT JOIN products p ON p.category_id = c.id WHERE c.id = $1;"
        ),
        "i32"
    );
}

/// A `NULL` fallback leaves `coalesce` nullable, and an argument the engine
/// can't type (a placeholder) leaves it untyped.
#[test]
fn coalesce_with_a_null_or_untyped_argument() {
    assert_eq!(
        scalar(
            "-- name: Note :one\nSELECT coalesce(note, NULL) AS note FROM products WHERE id = $1;"
        ),
        "Option<String>"
    );
    let reason = failure("-- name: Note :one\nSELECT coalesce(note, $1) AS note FROM products;");
    assert!(reason.contains("cannot infer the type"), "{reason}");
}

/// A comparison is `bool`, NULL only when an operand can be.
#[test]
fn comparisons_are_bool() {
    assert_eq!(
        scalar(
            "-- name: Expired :one\nSELECT expires_at <= now() AS expired FROM passes WHERE id = $1;"
        ),
        "bool"
    );
    assert_eq!(
        scalar(
            "-- name: Revoked :one\nSELECT revoked_at <= now() AS revoked FROM passes WHERE id = $1;"
        ),
        "Option<bool>"
    );
}

/// Boolean logic and the `IS`, `IN`, `BETWEEN` and `LIKE` tests are `bool`;
/// `IS [NOT] NULL` and `IS [NOT] DISTINCT FROM` never NULL.
#[test]
fn boolean_logic_is_bool() {
    let row = one("-- name: Flags :one\nSELECT \
             revoked_at IS NULL AS live, \
             revoked_at IS DISTINCT FROM expires_at AS changed, \
             NOT (seats > 0 AND label LIKE 'vip%') AS plain, \
             seats BETWEEN 1 AND 4 OR seats IN (8, 10) AS small, \
             label ILIKE $2 AS matched, \
             discount <> price AS discounted \
         FROM passes WHERE id = $1;");
    assert_eq!(
        row_columns(&row),
        [
            ("live", "bool"),
            ("changed", "bool"),
            ("plain", "bool"),
            ("small", "bool"),
            // The engine does not assume a parameter is never NULL.
            ("matched", "Option<bool>"),
            ("discounted", "Option<bool>"),
        ]
    );
}

/// Arithmetic takes its operands' type: integers widen to the larger, and
/// `numeric` wins over integers.
#[test]
fn arithmetic_takes_its_operands_type() {
    let row = one("-- name: Prices :one\nSELECT \
             price * 0.8 AS sale, \
             seats + 1 AS more, \
             seats * id AS wide, \
             -seats % 3 AS rest, \
             discount / seats AS share \
         FROM passes WHERE id = $1;");
    assert_eq!(
        row_columns(&row),
        [
            ("sale", "rust_decimal::Decimal"),
            ("more", "i32"),
            ("wide", "i64"),
            ("rest", "i32"),
            ("share", "Option<rust_decimal::Decimal>"),
        ]
    );
}

/// `CASE` takes the type its non-NULL branches share; a missing `ELSE` or a
/// NULL branch makes it nullable.
#[test]
fn case_takes_the_type_its_branches_share() {
    let row = one("-- name: Tiers :one\nSELECT \
             CASE WHEN seats > 4 THEN 'group' WHEN seats > 1 THEN label ELSE 'solo' END AS tier, \
             CASE seats WHEN 1 THEN price ELSE price * 2 END AS cost, \
             CASE WHEN seats > 4 THEN seats END AS big, \
             CASE WHEN revoked_at IS NULL THEN NULL ELSE seats + id END AS lost, \
             CASE WHEN seats > 4 THEN discount ELSE price END AS offer \
         FROM passes WHERE id = $1;");
    assert_eq!(
        row_columns(&row),
        [
            ("tier", "String"),
            ("cost", "rust_decimal::Decimal"),
            ("big", "Option<i16>"),
            ("lost", "Option<i64>"),
            ("offer", "Option<rust_decimal::Decimal>"),
        ]
    );
}

/// `||` is text.
#[test]
fn concatenation_is_text() {
    let row = one(
        "-- name: Labels :one\nSELECT label || ' (' || seats || ')' AS title, 'pass ' || revoked_at AS note \
         FROM passes WHERE id = $1;",
    );
    assert_eq!(
        row_columns(&row),
        [("title", "String"), ("note", "Option<String>")]
    );
}

/// Operands the engine cannot type, or types that do not combine, still
/// fail with the `-- column:` hint.
#[test]
fn mixed_or_unknown_operands_still_fail() {
    for expr in [
        "label + 1",
        "expires_at - now()",
        "seats * $2::text",
        "CASE WHEN seats > 1 THEN label ELSE seats END",
        "CASE WHEN seats > 1 THEN NULL END",
        "CASE WHEN seats > 1 THEN unknown_fn(seats) ELSE 1 END",
        "upper(label) + 1",
    ] {
        let reason = failure(&format!(
            "-- name: Q :one\nSELECT {expr} AS v FROM passes WHERE id = $1;"
        ));
        assert!(
            reason.contains("cannot infer the type of result column `v`")
                && reason.contains("add `-- column: v <RustType>`"),
            "{expr}: {reason}"
        );
    }
}

/// A call to a function the schema defines takes its `RETURNS` type, and
/// may be NULL unless a `-- column:` override says otherwise.
#[test]
fn schema_function_calls_take_their_return_type() {
    assert_eq!(
        scalar("-- name: Today :one\nSELECT tz_today() AS today;"),
        "Option<chrono::NaiveDate>"
    );
    assert_eq!(
        scalar(
            "-- name: Today :one\n-- column: today chrono::NaiveDate\nSELECT tz_today() AS today;"
        ),
        "chrono::NaiveDate"
    );
    assert_eq!(
        scalar(
            "-- name: Left :one\nSELECT app.seats_left(id, seats) AS left_over FROM passes WHERE id = $1;"
        ),
        "Option<i32>"
    );
    // Typed, it combines like any other operand.
    assert_eq!(
        scalar(
            "-- name: Due :one\nSELECT expires_at::date <= tz_today() AS due FROM passes WHERE id = $1;"
        ),
        "Option<bool>"
    );
}

#[test]
fn unknown_functions_and_unsupported_return_types_still_fail() {
    for call in ["nobody_defined_me()", "touch()", "public.seats_left(1, 2)"] {
        let reason = failure(&format!("-- name: Q :one\nSELECT {call} AS v;"));
        assert!(
            reason.contains("cannot infer the type of result column `v`")
                && reason.contains("add `-- column: v <RustType>`"),
            "{call}: {reason}"
        );
    }
}

/// Karibu Pass's partner cover: a cast to a column's own base type keeps
/// the column's typed id, here over a `coalesce` of two `media.id` columns,
/// one from a scalar subquery.
#[test]
fn a_cast_to_a_typed_ids_own_type_keeps_the_typed_id() {
    assert_eq!(
        typed_one(
            "-- name: Cover :one\nSELECT coalesce(l.cover_media_id, (SELECT ph.media_id FROM partner_photos ph WHERE ph.partner_id = l.partner_id ORDER BY ph.sort LIMIT 1))::uuid AS cover FROM listings l WHERE l.id = $1;"
        )
        .result,
        ResultShape::Scalar("Option<MediaId>".to_owned())
    );
    let row = typed_one(
        "-- name: Ids :one\nSELECT media_id::uuid AS media, media_id::text AS label, partner_id::bigint AS partner FROM partner_photos WHERE id = $1;",
    );
    assert_eq!(
        row_columns(&row),
        [
            ("media", "MediaId"),
            ("label", "String"),
            ("partner", "PartnerId"),
        ]
    );
}

/// `coalesce` takes the type its arguments share, and is never NULL when
/// one of them never is.
#[test]
fn coalesce_takes_the_type_its_arguments_share() {
    assert_eq!(
        scalar(
            "-- name: Note :one\nSELECT coalesce(note, sku) AS note FROM products WHERE id = $1;"
        ),
        "String"
    );
    assert_eq!(
        typed_one(
            "-- name: Cover :one\nSELECT coalesce(l.cover_media_id, (SELECT ph.media_id FROM partner_photos ph WHERE ph.partner_id = l.partner_id LIMIT 1)) AS cover FROM listings l WHERE l.id = $1;"
        )
        .result,
        ResultShape::Scalar("Option<MediaId>".to_owned())
    );
}

/// `EXISTS (…)` and `NOT EXISTS (…)` are a `bool` that is never NULL, and
/// the subquery's placeholders are typed from its own tables.
#[test]
fn exists_is_a_bool_that_is_never_null() {
    let row = one(
        "-- name: Photos :one\nSELECT (EXISTS (SELECT 1 FROM partner_photos ph WHERE ph.partner_id = p.id))::boolean AS has_photos, NOT EXISTS (SELECT 1 FROM partner_photos WHERE partner_id = p.id AND caption = $2) AS bare FROM partners p WHERE p.id = $1;",
    );
    assert_eq!(
        row_columns(&row),
        [("has_photos", "bool"), ("bare", "bool")]
    );
    assert_eq!(
        row.params,
        [param(1, "id", "i64"), param(2, "caption", "String")]
    );
}

/// `-- column:` names the generated typed ids and enums as `-- param:`
/// does: the query file imports them.
#[test]
fn a_column_override_names_generated_typed_ids_and_enums() {
    let (parsed, tables) = typed_tables();
    let (analyzed, failures) = analyze_against(
        "-- name: Card :one\n-- column: cover Option<MediaId>\n-- column: status types::ProductStatus\nSELECT coalesce(cover_media_id, NULL) AS cover, status::text AS status FROM listings WHERE id = $1;",
        &parsed,
        &tables,
    );
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(
        row_columns(&analyzed[0]),
        [
            ("cover", "Option<MediaId>"),
            ("status", "types::ProductStatus")
        ]
    );
    let rendered = crate::generator::queries::render(&analyzed, &tables, false);
    assert!(rendered.contains("::media::models::MediaId"), "{rendered}");
    assert!(rendered.contains("::types"), "{rendered}");
    assert!(
        rendered.contains("pub cover: Option<MediaId>,"),
        "{rendered}"
    );
    assert!(
        rendered.contains("pub status: types::ProductStatus,"),
        "{rendered}"
    );
}

/// A subquery's own tables come first: its unqualified columns and a name
/// it reuses resolve to them, and an outer table's column is still visible.
/// One the engine can't analyze stays untyped, as before, instead of
/// failing the query.
#[test]
fn subqueries_resolve_their_own_tables_first() {
    let query = one(
        "-- name: Latest :one\nSELECT (SELECT max(sort) FROM partner_photos WHERE partner_id = p.id AND caption = $2)::int AS last_sort FROM partner_photos p WHERE p.id = $1;",
    );
    assert_eq!(
        query.params,
        [param(1, "id", "uuid::Uuid"), param(2, "caption", "String")]
    );
    let shadowed = one(
        "-- name: Shadowed :many\nSELECT id FROM partners WHERE EXISTS (SELECT 1 FROM partners WHERE id = $1);",
    );
    assert_eq!(shadowed.params, [param(1, "id", "i64")]);
    let opaque = one(
        "-- name: Opaque :many\n-- param: $1 cutoff i64\nSELECT id FROM partners WHERE id IN (SELECT n FROM (SELECT $1 AS n) s);",
    );
    assert_eq!(opaque.params, [param(1, "cutoff", "i64")]);
}

/// Karibu Pass's "due Partners": one column from two tables, deduplicated,
/// with `ORDER BY` and `LIMIT` over the whole result.
#[test]
fn a_union_types_its_columns_from_every_branch() {
    let due = one(
        "-- name: DuePartners :many\nSELECT partner_id FROM offers WHERE status = $1\nUNION\nSELECT partner_id FROM partner_photos WHERE caption = $2\nORDER BY partner_id LIMIT $3;",
    );
    assert_eq!(due.result, ResultShape::Scalar("i64".to_owned()));
    assert_eq!(
        due.params,
        [
            param(1, "status", "String"),
            param(2, "caption", "String"),
            param(3, "limit", "i64"),
        ]
    );

    // Names come from the first branch; types are shared as `CASE`'s are
    // (`smallint` widens to `integer`), nullable when any branch is.
    let widened = one(
        "-- name: Holds :many\nSELECT id, rooms_held AS n, held_at FROM allocations\nUNION ALL\nSELECT id, seats, NULL::timestamptz FROM passes\nUNION ALL\nSELECT id, 0, now() FROM partners;",
    );
    assert_eq!(
        row_columns(&widened),
        [
            ("id", "i64"),
            ("n", "i32"),
            ("held_at", "Option<chrono::DateTime<chrono::Utc>>")
        ]
    );
    assert_eq!(
        scalar(
            "-- name: Q :many\nSELECT price FROM passes UNION SELECT rooms_held FROM allocations;"
        ),
        "rust_decimal::Decimal"
    );

    // Typed ids: one typed id stays, a typed id beside its plain type is
    // the plain type.
    assert_eq!(
        typed_one("-- name: Q :many\nSELECT id FROM partners UNION SELECT partner_id FROM offers;")
            .result,
        ResultShape::Scalar("PartnerId".to_owned())
    );
    assert_eq!(
        typed_one("-- name: Q :many\nSELECT id FROM partners UNION SELECT 0::bigint;").result,
        ResultShape::Scalar("i64".to_owned())
    );
}

/// `INTERSECT` keeps rows found in both branches, so a column is nullable
/// only when both are; `EXCEPT` keeps the left branch's rows.
#[test]
fn intersect_and_except_take_their_rows_nullability() {
    assert_eq!(
        scalar("-- name: Q :many\nSELECT note FROM products INTERSECT SELECT name FROM products;"),
        "String"
    );
    assert_eq!(
        scalar("-- name: Q :many\nSELECT note FROM products EXCEPT SELECT name FROM products;"),
        "Option<String>"
    );
    let except = one(
        "-- name: Idle :many\nSELECT id FROM partners EXCEPT SELECT partner_id FROM offers WHERE status = $1;",
    );
    assert_eq!(except.result, ResultShape::Scalar("i64".to_owned()));
    assert_eq!(except.params, [param(1, "status", "String")]);
}

#[test]
fn union_branches_must_match_column_by_column() {
    assert_eq!(
        failure("-- name: Q :many\nSELECT id, name FROM partners UNION SELECT id FROM categories;"),
        "the branches of UNION return 2 and 1 columns: column `name` has no match"
    );
    assert_eq!(
        failure(
            "-- name: Q :many\nSELECT id, name FROM partners UNION SELECT id, rooms_held FROM allocations;"
        ),
        "column `name` is `String` on the left of UNION and `i32` on its right; cast one of them"
    );
}

/// A set operation in a subquery sees the enclosing query's tables, and
/// one in a CTE or as an `INSERT … SELECT` source types like any other.
#[test]
fn set_operations_nest() {
    let correlated = one(
        "-- name: Pictured :many\nSELECT p.id FROM partners p WHERE EXISTS (\n    SELECT 1 FROM offers WHERE partner_id = p.id\n    UNION\n    SELECT 1 FROM partner_photos WHERE partner_id = p.id AND caption = $1\n);",
    );
    assert_eq!(correlated.params, [param(1, "caption", "String")]);

    let within = one(
        "-- name: Busy :many\nSELECT id, name FROM partners WHERE id IN (SELECT partner_id FROM offers WHERE status = $1 UNION SELECT partner_id FROM listings);",
    );
    assert_eq!(within.params, [param(1, "status", "String")]);

    let cte = one(
        "-- name: Ids :many\nWITH ids AS (SELECT partner_id AS id FROM offers UNION SELECT partner_id FROM listings)\nSELECT id FROM ids WHERE id > $1;",
    );
    assert_eq!(cte.params, [param(1, "id", "i64")]);

    let copy = one(
        "-- name: Copy :execrows\nINSERT INTO offers (partner_id, status)\nSELECT $1, 'copied'\nUNION\nSELECT partner_id, $2 FROM offers WHERE id = $3;",
    );
    assert_eq!(
        copy.params,
        [
            param(1, "partner_id", "i64"),
            param(2, "status", "String"),
            param(3, "id", "i64"),
        ]
    );
    assert!(
        failure(
            "-- name: Q :exec\nINSERT INTO offers (partner_id, status) SELECT $1, 'a' UNION SELECT partner_id FROM offers;"
        )
        .contains("the SELECT returns 1 values for 2 columns")
    );
}

/// `generate_series` in `FROM` yields one column, named by the alias (or
/// its column list), never NULL: integers give their widest type, a date or
/// timestamp series gives the timestamp PostgreSQL returns (`date` bounds
/// resolve to `timestamp with time zone`), and its step is an interval.
#[test]
fn generate_series_types_its_column() {
    let numbers = one("-- name: Numbers :many\nSELECT n FROM generate_series(1, $1) AS n;");
    assert_eq!(numbers.result, ResultShape::Scalar("i32".to_owned()));
    assert_eq!(numbers.params, [param(1, "stop", "i32")]);

    let weeks = one(
        "-- name: Weeks :many\nSELECT week, week::date AS day FROM generate_series($1::date, $2::date, $3) AS week;",
    );
    assert_eq!(
        row_columns(&weeks),
        [
            ("week", "chrono::DateTime<chrono::Utc>"),
            ("day", "chrono::NaiveDate")
        ]
    );
    assert_eq!(
        weeks.params,
        [
            param(1, "start", "chrono::NaiveDate"),
            param(2, "stop", "chrono::NaiveDate"),
            param(3, "step", "sqlx::postgres::types::PgInterval"),
        ]
    );

    let local = one(
        "-- name: Local :many\nSELECT g.at FROM generate_series($1::timestamp, $2::timestamp, '1 day') AS g(at);",
    );
    assert_eq!(
        local.result,
        ResultShape::Scalar("chrono::NaiveDateTime".to_owned())
    );

    let unaliased = one(
        "-- name: Unaliased :many\nSELECT generate_series FROM generate_series(1::bigint, 10);",
    );
    assert_eq!(unaliased.result, ResultShape::Scalar("i64".to_owned()));

    let numbered = one(
        "-- name: Numbered :many\nSELECT d, i FROM generate_series(1, 3) WITH ORDINALITY AS t(d, i);",
    );
    assert_eq!(row_columns(&numbered), [("d", "i32"), ("i", "i64")]);
}

/// Karibu's copy-forward: one row per week between two dates, cast back to
/// a date for the target column.
#[test]
fn generate_series_copies_forward_into_an_insert() {
    let query = one(
        "-- name: CopyForward :exec\nINSERT INTO weekly_holds (offer_id, week_of, rooms) SELECT $1, week::date, $3 FROM generate_series($2::date, $4::date, '7 days') AS week;",
    );
    assert_eq!(
        query.params,
        [
            param(1, "offer_id", "i64"),
            param(2, "start", "chrono::NaiveDate"),
            param(3, "rooms", "i32"),
            param(4, "stop", "chrono::NaiveDate"),
        ]
    );
}

/// `unnest` of a cast placeholder takes a `Vec<T>` named after its column,
/// and its column is `T`, never NULL; several arrays name their columns and
/// are nullable, since PostgreSQL pads the shorter ones with NULL.
#[test]
fn unnest_types_its_array_parameter_and_column() {
    let one_array = one(
        "-- name: Known :many\nSELECT id FROM products WHERE id IN (SELECT pid FROM unnest($1::uuid[]) AS pid);",
    );
    assert_eq!(one_array.params, [param(1, "pid_list", "Vec<uuid::Uuid>")]);

    let days = one("-- name: Days :many\nSELECT d.day FROM unnest($1::date[]) AS d(day);");
    assert_eq!(
        days.result,
        ResultShape::Scalar("chrono::NaiveDate".to_owned())
    );
    assert_eq!(
        days.params,
        [param(1, "day_list", "Vec<chrono::NaiveDate>")]
    );

    let batch = one(
        "-- name: AddItems :exec\nINSERT INTO order_items (order_id, item_id, quantity) SELECT $1, i.item_id, i.quantity FROM unnest($2::uuid[], $3::int[]) AS i(item_id, quantity);",
    );
    assert_eq!(
        batch.params,
        [
            param(1, "order_id", "uuid::Uuid"),
            param(2, "item_id_list", "Vec<uuid::Uuid>"),
            param(3, "quantity_list", "Vec<i32>"),
        ]
    );
    let padded = one(
        "-- name: Pairs :many\nSELECT a, b, n FROM unnest($1::int[], $2::text[]) WITH ORDINALITY AS t(a, b, n);",
    );
    assert_eq!(
        row_columns(&padded),
        [("a", "Option<i32>"), ("b", "Option<String>"), ("n", "i64")]
    );
}

/// `jsonb_to_recordset($n) AS x(col type, …)` types its columns from the
/// definition list, nullable (a missing key is NULL), and `$n` as a
/// `serde_json::Value` named after the alias; `json_to_recordset` too.
#[test]
fn jsonb_to_recordset_types_its_columns_and_json_parameter() {
    let insert = one(
        "-- name: AddLines :exec\nINSERT INTO order_items (order_id, item_id, quantity) SELECT $1, l.item_id, l.quantity FROM jsonb_to_recordset($2) AS l(item_id uuid, quantity int);",
    );
    assert_eq!(
        insert.params,
        [
            param(1, "order_id", "uuid::Uuid"),
            param(2, "l", "serde_json::Value"),
        ]
    );

    let read = one(
        "-- name: Lines :many\n-- param: $1 lines\nSELECT item_id, quantity FROM json_to_recordset($1::json) AS x(item_id uuid, quantity int);",
    );
    assert_eq!(
        row_columns(&read),
        [
            ("item_id", "Option<uuid::Uuid>"),
            ("quantity", "Option<i32>")
        ]
    );
    assert_eq!(read.params, [param(1, "lines", "serde_json::Value")]);
}

/// What the engine can't type in `FROM` says what to write instead.
#[test]
fn set_returning_functions_explain_what_they_need() {
    assert!(
        failure("-- name: Bare :many\nSELECT * FROM jsonb_to_recordset($1) AS l;")
            .contains("needs a column definition list"),
    );
    assert!(
        failure("-- name: Untyped :many\nSELECT * FROM unnest($1) AS u;")
            .contains("cast it to an array"),
    );
    assert!(
        failure("-- name: Unknown :many\nSELECT * FROM generate_series($1, $2) AS g;")
            .contains("cast a bound"),
    );
    assert!(
        failure("-- name: NoStep :many\nSELECT * FROM generate_series($1::date, $2::date) AS g;")
            .contains("needs a step"),
    );
    assert!(
        failure("-- name: Other :many\nSELECT * FROM json_each($1) AS j;")
            .contains("function `json_each` in FROM is not supported"),
    );
    assert!(
        failure("-- name: Unnamed :many\nSELECT * FROM unnest($1::int[], $2::int[]) AS t;")
            .contains("needs its columns named"),
    );
    assert!(
        failure("-- name: Small :many\nSELECT * FROM generate_series(1::smallint, $1) AS g;")
            .contains("ambiguous"),
    );
    assert_eq!(
        one("-- name: Widened :many\nSELECT g FROM generate_series(1::smallint, 3) AS g;").result,
        ResultShape::Scalar("i32".to_owned())
    );
}

/// `-- returns:` reuses another query's row type when the columns match,
/// and names each difference when they don't.
#[test]
fn returns_reuses_a_row_type_of_the_file() {
    let (analyzed, failures) = analyze(
        "-- name: FindProduct :one\nSELECT id, sku FROM products WHERE id = $1;\n\
         -- name: ProductsBySku :many\n-- returns: FindProductRow\nSELECT id, sku FROM products WHERE sku = $1;\n",
    );
    assert!(failures.is_empty(), "{failures:?}");
    let ResultShape::Shared { name, columns } = &analyzed[1].result else {
        panic!("not shared: {:?}", analyzed[1].result);
    };
    assert_eq!(name, "FindProductRow");
    assert_eq!(columns.len(), 2);

    let (_, failures) = analyze(
        "-- name: FindProduct :one\nSELECT id, sku FROM products WHERE id = $1;\n\
         -- name: Others :many\n-- returns: FindProductRow\nSELECT sku, id, stock FROM products;\n",
    );
    let reason = &failures[0].reason;
    assert!(
        reason.contains("column 1 is `sku` here and `id` in `FindProductRow`"),
        "{reason}"
    );
    assert!(
        reason.contains("column `stock` is not in `FindProductRow`"),
        "{reason}"
    );

    let (_, failures) = analyze(
        "-- name: FindProduct :one\nSELECT id, sku FROM products WHERE id = $1;\n\
         -- name: Retyped :many\n-- returns: FindProductRow\nSELECT id, stock AS sku FROM products;\n",
    );
    let reason = &failures[0].reason;
    assert!(
        reason.contains("column `sku` is `i32` here and `String` in `FindProductRow`"),
        "{reason}"
    );
}

/// `-- returns:` names a model when the columns are its table's, or a
/// hand-written type by path, which decodes as it is.
#[test]
fn returns_names_a_model_or_a_hand_written_type() {
    let model =
        one("-- name: Named :many\n-- returns: Category\nSELECT id, name FROM categories;\n");
    assert_eq!(
        model.result,
        ResultShape::Model {
            module: "categories".to_owned(),
            struct_name: "Category".to_owned()
        }
    );
    let external = one(
        "-- name: Summary :many\n-- returns: crate::features::payouts::dto::PayoutSummary\n\
         SELECT id, sku FROM products;\n",
    );
    assert_eq!(
        external.result,
        ResultShape::External("crate::features::payouts::dto::PayoutSummary".to_owned())
    );
    let reason =
        failure("-- name: Count :one\n-- returns: CountRow2\nSELECT count(*) FROM products;\n");
    assert!(reason.contains("returns one column"), "{reason}");
    let reason =
        failure("-- name: Nope :many\n-- returns: Nowhere\nSELECT id, sku FROM products;\n");
    assert!(
        reason.contains("no query of this file returns `Nowhere`"),
        "{reason}"
    );
}

/// Karibu Pass's `CancelEvent` shape: a data-modifying CTE whose
/// `UPDATE … FROM` reads a subquery, typed from its own query as a CTE is.
const CANCEL_EVENT: &str = "WITH e AS (\n    UPDATE offers SET status = 'cancelled' WHERE id = $1 RETURNING id\n),\nt AS (\n    UPDATE allocations SET rooms_held = allocations.rooms_held - x.held\n    FROM (SELECT offer_id, sum(rooms)::int AS held FROM weekly_holds\n          WHERE offer_id IN (SELECT id FROM e) AND week_of >= $2 GROUP BY offer_id) x\n    WHERE allocations.offer_id = x.offer_id\n    RETURNING allocations.id, x.held\n)\nSELECT id, held FROM t";

#[test]
fn a_subquery_in_from_acts_as_a_table_typed_from_its_query() {
    let query = one(&format!("-- name: CancelEvent :many\n{CANCEL_EVENT};"));
    assert_eq!(
        query.params,
        [
            param(1, "id", "i64"),
            param(2, "week_of", "chrono::NaiveDate")
        ]
    );
    // `sum` is null over no rows, so `held` stays optional.
    assert_eq!(
        row_columns(&query),
        [("id", "i64"), ("held", "Option<i32>")]
    );

    let select = one(
        "-- name: Held :many\nSELECT o.status, x.total FROM offers o JOIN (SELECT offer_id, count(*) AS total FROM weekly_holds GROUP BY offer_id) AS x ON x.offer_id = o.id WHERE x.total > $1;",
    );
    assert_eq!(select.params, [param(1, "total", "i64")]);
    assert_eq!(
        row_columns(&select),
        [("status", "String"), ("total", "i64")]
    );

    let delete = one(
        "-- name: DropEmpty :execrows\nDELETE FROM allocations USING (SELECT id FROM offers WHERE status = $1) gone WHERE allocations.offer_id = gone.id;",
    );
    assert_eq!(delete.params, [param(1, "status", "String")]);
}

/// Only a `LATERAL` subquery sees the `FROM` tables beside it, as in
/// PostgreSQL; the queries around its own stay visible either way.
#[test]
fn only_a_lateral_subquery_sees_the_tables_beside_it() {
    let latest = one(
        "-- name: LatestHolds :many\nSELECT o.id, h.week_of FROM offers o, LATERAL (SELECT week_of FROM weekly_holds w WHERE w.offer_id = o.id AND w.rooms > $1 ORDER BY week_of DESC LIMIT 1) h;",
    );
    assert_eq!(latest.params, [param(1, "rooms", "i32")]);
    assert_eq!(
        row_columns(&latest),
        [("id", "i64"), ("week_of", "chrono::NaiveDate")]
    );

    assert_eq!(
        failure(
            "-- name: Q :many\nSELECT o.id, h.week_of FROM offers o, (SELECT week_of FROM weekly_holds w WHERE w.offer_id = o.id) h;"
        ),
        "`o` is a FROM table beside the subquery, which can't see it; write `LATERAL (SELECT …)` to refer to it"
    );
    assert!(
        failure(
            "-- name: Q :execrows\nUPDATE allocations SET rooms_held = 0 FROM (SELECT id FROM offers WHERE offers.id = allocations.offer_id) x;"
        )
        .contains("`allocations` is a FROM table beside the subquery")
    );
}

/// A subquery's alias may list its column names, as a CTE's may.
#[test]
fn a_subquery_alias_column_list_renames_its_columns() {
    let query = one(
        "-- name: Totals :many\nSELECT cat, n FROM (SELECT category_id, count(*) FROM products GROUP BY category_id) AS t (cat, n) WHERE n > $1;",
    );
    assert_eq!(query.params, [param(1, "n", "i64")]);
    assert_eq!(row_columns(&query), [("cat", "i64"), ("n", "i64")]);

    assert_eq!(
        failure("-- name: Q :many\nSELECT a FROM (SELECT id FROM categories) t (a, b);"),
        "subquery `t` names 2 columns but its query returns 1"
    );
    assert!(
        failure("-- name: Q :many\nSELECT category_id FROM (SELECT category_id FROM products) t (category_id) WHERE t.id = $1;")
            .contains("column `id` does not exist in `t`")
    );
}
