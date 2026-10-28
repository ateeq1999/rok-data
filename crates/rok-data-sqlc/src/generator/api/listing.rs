//! `db/<table>/listing.rs`: the reads and writes an API feature needs
//! beyond the data layer's own, kept with the rest of the SQL (engine ADR 0016).
//! Generated for a table whose feature serves `list` (the `XSort` keys
//! `sort` takes, and a page of rows matching `q`, with its count) or
//! `search`, has an owner (every read scoped to the caller: the list's,
//! `get_owned` and `lock_owned`), or writes a many-to-many relationship
//! (its join table's writes, which a feature may only call through its own
//! table's module).

use super::feature::{Endpoint, Feature};
use super::layout::push_signature;
use super::relations::Relation;
use crate::generator::openapi::push_to_schema;
use crate::generator::sql::{
    column_clause, first_placeholder, ident, live_key_predicate, scoped_key_predicate, str_literal,
    table_ref,
};
use crate::generator::{
    HEADER, StaticQuery, doc_name, keys, model_chain, param_name, param_type, push_doc,
    push_fn_head, push_wide_chain, types_import, use_paths,
};
use crate::schema::Column;

/// The `search.rs` helper a cursor-paged list matches `q` with, appended
/// to `db/search.rs` when `api.cursor` names a table.
pub const CONTAINS_PATTERN: &str = r#"
/// The `ILIKE` pattern of the text containing `text`, ignoring case: `%`,
/// `_` and `\` in `text` match themselves. The API's cursor-paged lists
/// match `q` with it.
#[must_use]
pub fn contains_pattern(text: &str) -> String {
    format!("%{}%", escape_like(text))
}
"#;

/// Renders `listing.rs` for `feature` (see [`super::has_listing`]).
pub fn render(feature: &Feature<'_>) -> String {
    let table = feature.table;
    let lists = feature.serves(Endpoint::List);
    let searches = feature.serves(Endpoint::Search);
    let searched = !feature.search.is_empty();
    let owned = feature.reads_owned_rows();
    let pairs: Vec<&Relation<'_>> = feature
        .relations
        .iter()
        .filter(|relation| relation.writes().is_some())
        .collect();
    let mut docs = Vec::new();
    if lists {
        docs.push(format!(
            "//! What the API's list endpoint reads: a page of rows{}{}, sorted by\n//! one of the [`{}`] keys.\n",
            if searched { " matching `q`" } else { "" },
            if feature.owner.is_some() {
                " the caller owns"
            } else {
                ""
            },
            feature.sort_enum()
        ));
    }
    if searches {
        docs.push(
            "//! What `POST /search` reads: a page of the rows a filter matches, and\n//! their count.\n"
                .to_owned(),
        );
    }
    if owned {
        docs.push(
            "//! The reads of one row the caller owns: another caller's row is none.\n".to_owned(),
        );
    }
    let checks = &feature.unique_checks;
    if !checks.is_empty() {
        docs.push(
            "//! The `unique` checks of a create or update body: whether another row\n//! already has its values.\n"
                .to_owned(),
        );
    }
    if !pairs.is_empty() {
        docs.push(
            "//! The many-to-many relationships' writes, through their join tables.\n".to_owned(),
        );
    }
    let mut output = format!("{HEADER}\n{}\n", docs.join("//!\n"));

    let mut paths = Vec::new();
    let cursor = lists && feature.cursor;
    if lists || searches {
        // In rustfmt's order: the types, then the functions.
        // Every sort enum orders by `direction`.
        let mut search_items = vec!["Direction", "Sort"];
        if searches {
            search_items.push("Window");
        }
        if cursor && searched {
            search_items.push("contains_pattern");
        }
        paths.push(format!(
            "super::super::search::{{{}}}",
            search_items.join(", ")
        ));
    }
    if searches {
        paths.push(format!("super::filter::{}", feature.db_filter()));
    }
    if lists {
        // The chain starts at the model: `Note::query()`, `Note::filter(…)`.
        paths.push("rok_data::Model".to_owned());
    }
    let mut models = vec![feature.row().to_owned()];
    if owned || !checks.is_empty() {
        models.extend(keys::key_items(table));
        paths.extend(keys::id_imports(table, keys::param_columns(table), true));
        let checked = || {
            checks
                .iter()
                .flat_map(|check| check.columns.iter().copied())
        };
        paths.extend(keys::id_imports(table, checked(), true));
        models.extend(
            checked()
                .filter_map(|column| column.typed_id.as_ref())
                .filter(|id| id.module == table.module_name)
                .map(|id| id.name.clone()),
        );
        let key_type = keys::key_type(table);
        let types = checked().map(Column::value_type).chain([key_type.as_str()]);
        if !types_import("super::super", types).is_empty() {
            paths.push("super::super::types".to_owned());
        }
    }
    for writes in pairs.iter().filter_map(|relation| relation.writes()) {
        paths.extend(keys::id_imports(table, writes.join.tenant(), true));
        for column in [writes.own, writes.other] {
            if let Some(id) = &column.typed_id {
                if id.module == table.module_name {
                    models.push(id.name.clone());
                } else {
                    paths.push(format!("super::super::{}::models::{}", id.module, id.name));
                }
            }
        }
    }
    models.sort();
    models.dedup();
    let models = if let [model] = models.as_slice() {
        model.clone()
    } else {
        format!("{{{}}}", models.join(", "))
    };
    paths.push(format!("super::models::{models}"));
    output.push_str(&use_paths(&paths));

    if lists || searches {
        output.push('\n');
        render_sort_enum(&mut output, feature);
    }
    if cursor {
        super::cursor_list::render_listing(&mut output, feature);
    } else if lists {
        render_list(&mut output, feature);
    }
    if searches {
        super::search::render_listing(&mut output, feature);
    }
    if owned {
        output.push('\n');
        render_owned(&mut output, feature);
    }
    super::unique::render_reads(&mut output, feature, checks);
    for relation in pairs {
        super::relation_service::render_pair_writes(&mut output, relation);
    }
    output
}

/// The read of one row the caller owns, `get_owned`, `lock_owned`, the
/// check a write runs first, and `owned_by`, `owned` and `retain_owned`,
/// the checks of rows read some other way.
fn render_owned(output: &mut String, feature: &Feature<'_>) {
    let table = feature.table;
    let Some(owner) = feature.owner else {
        return;
    };
    let keys = table.key_columns();
    let owner_condition = format!(
        "{} = ${}",
        ident(&owner.sql_name),
        first_placeholder(table) + keys.len()
    );
    let predicate = format!("{} AND {owner_condition}", live_key_predicate(table, &keys));
    let mut params = keys::key_params(table);
    params.push("owner: uuid::Uuid".to_owned());
    let mut binds = keys::scoped_key_binds(table);
    binds.push("owner".to_owned());

    let mut sql = column_clause("SELECT", table);
    sql.push(format!("FROM {}", table_ref(table)));
    sql.push(format!("WHERE {predicate}"));
    push_doc(
        output,
        0,
        &[
            &format!(
                "Fetches the row with the given primary key if `owner` owns it (its {}),",
                doc_name(&owner.sql_name)
            ),
            "or `None`.",
        ],
    );
    push_fn_head(
        output,
        "get_owned",
        &params,
        &format!("Result<Option<{}>, sqlx::Error>", feature.row()),
    );
    StaticQuery {
        call: &format!("sqlx::query_as::<_, {}>", feature.row()),
        sql,
        binds: binds.clone(),
        execute: "fetch_optional(executor)",
    }
    .push(output, 4, "", "");
    output.push_str("}\n\n");

    // Soft-deleted rows too: restoring one checks its owner first.
    let sql = vec![
        "SELECT 1".to_owned(),
        format!("FROM {}", table_ref(table)),
        format!(
            "WHERE {} AND {owner_condition}",
            scoped_key_predicate(table, &keys)
        ),
        "FOR UPDATE".to_owned(),
    ];
    let mut doc = vec![
        "Returns whether `owner` owns the row with the given primary key, locking",
        "it until the transaction ends, so a write that checks first changes only",
        "a row the caller owns.",
    ];
    if table.soft_delete_column.is_some() {
        doc.push("A soft-deleted row counts.");
    }
    push_doc(output, 0, &doc);
    push_fn_head(output, "lock_owned", &params, "Result<bool, sqlx::Error>");
    StaticQuery {
        call: "sqlx::query_scalar::<_, i32>",
        sql,
        binds: binds.clone(),
        execute: "fetch_optional(executor)",
    }
    .push(output, 4, "let row = ", "?;");
    output.push_str("    Ok(row.is_some())\n}\n\n");

    let sql = vec![
        "SELECT EXISTS (".to_owned(),
        "    SELECT 1".to_owned(),
        format!("    FROM {}", table_ref(table)),
        format!("    WHERE {predicate}"),
        ")".to_owned(),
    ];
    let mut doc = vec![
        "Returns whether `owner` owns the row with the given primary key: the",
        "`exists` check of a reference to it in a create or update body.",
    ];
    if table.soft_delete_column.is_some() {
        doc.push("Soft-deleted rows are left out.");
    }
    push_doc(output, 0, &doc);
    push_fn_head(output, "exists_owned", &params, "Result<bool, sqlx::Error>");
    StaticQuery {
        call: "sqlx::query_scalar::<_, bool>",
        sql,
        binds,
        execute: "fetch_one(executor)",
    }
    .push(output, 4, "", "");
    output.push_str("}\n");

    let row = feature.row();
    let owner_value = if owner.typed_id.is_some() {
        format!("uuid::Uuid::from(row.{})", owner.rust_name)
    } else {
        format!("row.{}", owner.rust_name)
    };
    let owner_param = "owner: uuid::Uuid".to_owned();
    output.push_str(&format!(
        "\n/// Whether `owner` owns `row` (its {}).\n#[must_use]\n",
        doc_name(&owner.sql_name)
    ));
    let params = [format!("row: &{row}"), owner_param.clone()];
    push_signature(output, 0, "pub fn owned_by", &params, "bool");
    output.push_str(&format!("    {owner_value} == owner\n}}\n"));

    output.push_str("\n/// `row`, if `owner` owns it.\n#[must_use]\n");
    let params = [format!("row: Option<{row}>"), owner_param.clone()];
    let returns = format!("Option<{row}>");
    push_signature(output, 0, "pub fn owned", &params, &returns);
    output.push_str("    row.filter(|row| owned_by(row, owner))\n}\n");

    output.push_str("\n/// Keeps the rows `owner` owns.\n");
    let params = [format!("rows: &mut Vec<{row}>"), owner_param];
    push_signature(output, 0, "pub fn retain_owned", &params, "");
    output.push_str("    rows.retain(|row| owned_by(row, owner));\n}\n");
}

/// `SEARCH_COLUMNS` and `matching`: the list endpoint's rows as a query
/// chain, which the service runs a page at a time (`paginate`).
fn render_list(output: &mut String, feature: &Feature<'_>) {
    let table = feature.table;
    let row = feature.row();
    let sort = feature.sort_enum();
    let searched = !feature.search.is_empty();
    if searched {
        let columns: Vec<String> = feature
            .search
            .iter()
            .map(|column| column_const(row, column))
            .collect();
        output.push_str("\n/// The columns `q` matches.\n");
        let head = format!(
            "const SEARCH_COLUMNS: [rok_data::Column<{row}>; {}] = ",
            columns.len()
        );
        let one_line = format!("{head}[{}];", columns.join(", "));
        if one_line.len() <= crate::generator::MAX_WIDTH {
            output.push_str(&one_line);
            output.push('\n');
        } else {
            output.push_str(&format!("{head}[\n"));
            for column in &columns {
                output.push_str(&format!("    {column},\n"));
            }
            output.push_str("];\n");
        }
    }

    let mut params: Vec<String> = table
        .tenant()
        .map(|tenant| format!("{}: {}", param_name(tenant), param_type(tenant)))
        .into_iter()
        .collect();
    params.extend(feature.owner.map(|_| "owner: uuid::Uuid".to_owned()));
    params.extend(searched.then(|| "q: Option<&str>".to_owned()));
    params.push(format!("sort: Option<Sort<{sort}>>"));

    output.push_str(&format!("\nimpl {row} {{\n"));
    let matching = format!(
        "{}{}",
        if feature.owner.is_some() {
            " `owner` owns"
        } else {
            ""
        },
        if searched { " matching `q`" } else { "" }
    );
    let keys = table.primary_key_columns();
    let mut doc = vec![
        format!(
            "The rows of the list endpoint{matching}, ordered by `sort`{}.",
            if keys.is_empty() {
                ""
            } else {
                ", ties by primary key"
            }
        ),
        "Run it a page at a time with `.paginate(executor, page, per_page)`.".to_owned(),
    ];
    if table.soft_delete_column.is_some() {
        doc.push("Soft-deleted rows are left out.".to_owned());
    }
    if searched {
        doc.push(
            "`q` matches a row when one of the [`SEARCH_COLUMNS`] contains it, ignoring".to_owned(),
        );
        doc.push("case; `%`, `_` and `\\` in it match themselves.".to_owned());
    }
    let doc: Vec<&str> = doc.iter().map(String::as_str).collect();
    push_doc(output, 4, &doc);
    output.push_str("    #[must_use]\n");
    push_signature(
        output,
        4,
        "pub fn matching",
        &params,
        &format!("rok_data::Select<{row}>"),
    );
    let mut scope = Vec::new();
    if let Some(tenant) = table.tenant() {
        let value = param_name(tenant);
        scope.push(format!(
            ".filter({}.eq({value}))",
            column_const(row, tenant)
        ));
    }
    if let Some(owner) = feature.owner {
        scope.push(format!(".filter({}.eq(owner))", column_const(row, owner)));
    }
    let lead = "let mut query = ";
    let head = model_chain(row, &mut scope, 8, lead);
    push_wide_chain(output, 8, lead, &head, &scope, ";");
    if searched {
        output.push_str(
            "        if let Some(q) = q {\n            query = query.filter_any(SEARCH_COLUMNS, |column| column.icontains(q));\n        }\n",
        );
    }
    output.push_str(
        "        if let Some(sort) = sort {\n            query = query.order_by(sort.key.order(sort.direction));\n        }\n",
    );
    let ties: Vec<String> = keys
        .iter()
        .map(|key| format!(".order_by({})", column_const(row, key)))
        .collect();
    push_wide_chain(output, 8, "", "query", &ties, "");
    output.push_str("    }\n}\n");
}

/// `Row::COLUMN`: the model's constant for `column`.
fn column_const(row: &str, column: &Column) -> String {
    format!("{row}::{}", column.rust_name.to_uppercase())
}

/// The `XSort` enum: the columns `sort` takes, its default, and the column
/// each key sorts by.
fn render_sort_enum(output: &mut String, feature: &Feature<'_>) {
    let sort = feature.sort_enum();
    push_doc(
        output,
        0,
        &[&format!(
            "The columns `GET {}?sort=` takes, in `snake_case`.",
            feature.url
        )],
    );
    output.push_str(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]\n",
    );
    push_to_schema(output, feature.openapi);
    output.push_str("#[serde(rename_all = \"snake_case\")]\n");
    output.push_str(&format!("pub enum {sort} {{\n"));
    for column in &feature.sort {
        push_doc(
            output,
            4,
            &[&format!("Sorts by the `{}` column.", column.sql_name)],
        );
        output.push_str(&format!("    {},\n", column.sort_variant()));
    }
    output.push_str("}\n\n");
    output.push_str(&format!("impl {sort} {{\n"));
    if let Some(default) = feature.default_sort() {
        push_doc(
            output,
            4,
            &["The key a list sorts by when its query names none."],
        );
        output.push_str(&format!(
            "    pub const DEFAULT: Self = Self::{};\n\n",
            default.sort_variant()
        ));
    }
    push_doc(output, 4, &["The SQL column this key sorts by."]);
    output.push_str(
        "    #[must_use]\n    pub fn column(self) -> &'static str {\n        match self {\n",
    );
    for column in &feature.sort {
        output.push_str(&format!(
            "            Self::{} => {},\n",
            column.sort_variant(),
            str_literal(&ident(&column.sql_name))
        ));
    }
    output.push_str("        }\n    }\n");
    if feature.serves(Endpoint::List) && !feature.cursor {
        let row = feature.row();
        output.push('\n');
        push_doc(output, 4, &["How this key orders rows, in `direction`."]);
        output.push_str("    #[must_use]\n");
        let params = ["self".to_owned(), "direction: Direction".to_owned()];
        let returns = format!("rok_data::Order<{row}>");
        push_signature(output, 4, "pub fn order", &params, &returns);
        output.push_str("        let column = match self {\n");
        for column in &feature.sort {
            let arm = format!(
                "Self::{} => {},",
                column.sort_variant(),
                column_const(row, column)
            );
            if 12 + arm.len() <= crate::generator::MAX_WIDTH {
                output.push_str(&format!("            {arm}\n"));
            } else {
                // rustfmt wraps an overlong arm's body in a block.
                output.push_str(&format!(
                    "            Self::{} => {{\n                {}\n            }}\n",
                    column.sort_variant(),
                    column_const(row, column)
                ));
            }
        }
        output.push_str(
            "        };\n        match direction {\n            Direction::Asc => column.asc(),\n            Direction::Desc => column.desc(),\n        }\n    }\n",
        );
    }
    if feature.serves(Endpoint::Search) {
        output.push('\n');
        push_doc(
            output,
            4,
            &["The key `name` names, in `snake_case`, as a search body's `sort` does."],
        );
        output.push_str(
            "    #[must_use]\n    pub fn from_name(name: &str) -> Option<Self> {\n        match name {\n",
        );
        for column in &feature.sort {
            let variant = column.sort_variant();
            output.push_str(&format!(
                "            {} => Some(Self::{variant}),\n",
                str_literal(&super::search::serde_snake_case(&variant))
            ));
        }
        output.push_str("            _ => None,\n        }\n    }\n");
    }
    if feature.cursor && feature.serves(Endpoint::List) {
        super::cursor_list::render_order(output, feature);
    }
    output.push_str("}\n");
}
