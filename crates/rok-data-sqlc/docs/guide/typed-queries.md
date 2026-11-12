# What typed queries support

rok-data-sqlc types a query file without a database: it reads each
statement against the schema and works out every parameter's and every
result column's Rust type. This page lists the SQL forms it understands,
what to write when it can't type one, and the test that proves each row.
The README's [Writing query files](../../README.md#writing-query-files)
explains the rules in full.

When a form isn't typed, generation fails for that query and the error
names the fix:

- a result column: ``cannot infer the type of result column `v`; add
  `-- column: v <RustType>` ``. The override settles the type and its
  nullability.
- a parameter: ``cannot infer the type of `$1` from its context; add
  `-- param: $1 <name> <RustType>` ``.
- a column of a CTE or a `FROM` subquery: ``cannot infer the type of
  column `total` of CTE `a`; cast it explicitly (`(expr)::type AS
  total`)``. `-- column:` reaches only the query's own result columns, so
  a cast is the fix there.
- a query you'd rather write by hand: `-- rok:skip [<note>]` leaves it
  ungenerated, with a note instead of a failure.

A cast (`expr::type`) types any expression, as a result column or a
parameter, so it is the other way out of every row below.

"Typed" means the engine infers the type with no annotation. "Partly"
means some shapes are typed and the rest fail with the hint in the row.
Tests in `src/` run with `cargo test -p rok-data-sqlc`; the `fixture-app`
and `examples/api` ones run against PostgreSQL.

## Result columns

| SQL form | Typed | When it isn't | Test |
|---|---|---|---|
| Columns, `*`, `t.*`, aliases, joins (the outer side of a `LEFT`/`RIGHT`/`FULL` join is `Option`) | Yes | An unknown table or column, or an ambiguous one, fails naming it | [`left_joins_make_the_right_side_nullable`](../../src/queries/analyze/tests.rs), [`reports_schema_mismatches`](../../src/queries/analyze/tests.rs) |
| Casts (`expr::type`): the target type, with the wrapped expression's nullability; a cast to a typed id's own type keeps the id | Yes | A cast to a type the engine doesn't map fails: ``type `point` is not supported yet`` | [`a_cast_not_null_column_is_not_null`](../../src/queries/analyze/tests.rs), [`a_cast_outer_join_column_stays_nullable`](../../src/queries/analyze/tests.rs), [`a_cast_to_a_typed_ids_own_type_keeps_the_typed_id`](../../src/queries/analyze/tests.rs) |
| `coalesce(…)`: the type its arguments share, never NULL when one argument never is | Partly | A placeholder argument (`coalesce(note, $1)`) fails with `-- column:` | [`coalesce_takes_the_type_its_arguments_share`](../../src/queries/analyze/tests.rs), [`coalesce_with_a_literal_fallback_is_not_null`](../../src/queries/analyze/tests.rs), [`coalesce_with_a_null_or_untyped_argument`](../../src/queries/analyze/tests.rs) |
| Comparisons, `AND`/`OR`/`NOT`, `BETWEEN`, `IN (…)`, `LIKE`/`ILIKE`, regex matches: `bool`; `IS [NOT] NULL` and `IS [NOT] DISTINCT FROM` never NULL | Yes | | [`comparisons_are_bool`](../../src/queries/analyze/tests.rs), [`boolean_logic_is_bool`](../../src/queries/analyze/tests.rs) |
| Arithmetic `+ - * / %` and unary minus: integers widen, `numeric` beats integers, floats beat `numeric` | Partly | Operands that don't combine (`label + 1`, `expires_at - now()`) fail with `-- column:` | [`arithmetic_takes_its_operands_type`](../../src/queries/analyze/tests.rs), [`mixed_or_unknown_operands_still_fail`](../../src/queries/analyze/tests.rs) |
| Arithmetic with a parameter (`price * $1`): typed like `price * price`, nullable when the column is | Yes | A placeholder with no numeric partner (`$1 * 2`) fails with `-- param:` | [`arithmetic_with_a_placeholder_takes_the_columns_type`](../../src/queries/analyze/tests.rs) |
| `CASE`: the type its non-NULL branches share; `Option` with a NULL branch or no `ELSE` | Partly | Branches of different types (`text` and `integer`), or only NULL branches, fail with `-- column:` | [`case_takes_the_type_its_branches_share`](../../src/queries/analyze/tests.rs), [`mixed_or_unknown_operands_still_fail`](../../src/queries/analyze/tests.rs) |
| `\|\|`: `String` | Yes | | [`concatenation_is_text`](../../src/queries/analyze/tests.rs) |
| Literals: `i32`, `i64` when it doesn't fit, `Decimal` with a decimal point, `String` quoted | Yes, aliased | An unaliased expression fails: it needs an alias | [`arithmetic_takes_its_operands_type`](../../src/queries/analyze/tests.rs), [`reports_unsupported_statement_shapes`](../../src/queries/analyze/tests.rs) |
| `count(…)` (with `FILTER` too): `i64`, never NULL | Yes | | [`a_cast_count_is_not_null`](../../src/queries/analyze/tests.rs), [`a_cast_filtered_count_is_not_null`](../../src/queries/analyze/tests.rs) |
| `sum`, `avg`, `min`, `max` by PostgreSQL's rules, `Option` unless wrapped in `coalesce(…, non-null)`, in a `SELECT`, a CTE and a subquery alike; a cast still decides the type | Yes | An argument PostgreSQL can't aggregate (`sum(name)`) fails with `-- column:` (or the CTE's cast hint) | [`aggregates_take_postgres_result_types`](../../src/queries/analyze/tests.rs), [`coalesce_makes_an_aggregate_non_null`](../../src/queries/analyze/tests.rs), [`a_cast_decides_an_aggregates_type`](../../src/queries/analyze/tests.rs), [`untypeable_aggregates_still_fail`](../../src/queries/analyze/tests.rs) |
| `EXISTS (…)`, `NOT EXISTS (…)`: `bool`, never NULL; `x IN (SELECT …)`: `bool` | Yes | | [`exists_is_a_bool_that_is_never_null`](../../src/queries/analyze/tests.rs) |
| A scalar subquery `(SELECT …)`: its column's type, non-null only when it always returns one row (an aggregate, no `GROUP BY`, `HAVING`, `LIMIT`, `OFFSET` or set operation) and its column is non-null | Yes | Otherwise it is `Option`; `-- column: total i64` says it isn't | [`a_one_row_subquery_keeps_its_columns_nullability`](../../src/queries/analyze/tests.rs), [`a_one_row_subquery_over_a_nullable_aggregate_stays_optional`](../../src/queries/analyze/tests.rs) |

## Functions

| SQL form | Typed | When it isn't | Test |
|---|---|---|---|
| A function the schema defines, in `SELECT`: its `RETURNS` type, as `Option` | Yes | `-- column: today chrono::NaiveDate` drops the `Option`; a `RETURNS` type the engine doesn't map (`trigger`, `void`, `TABLE (…)`) or overloads with different types fail with `-- column:` | [`schema_function_calls_take_their_return_type`](../../src/queries/analyze/tests.rs), [`unknown_functions_and_unsupported_return_types_still_fail`](../../src/queries/analyze/tests.rs) |
| Clock functions in `SELECT` (`now()`, `current_timestamp`, `current_date`, `localtimestamp`, `localtime`): never NULL | Yes | | [`comparisons_are_bool`](../../src/queries/analyze/tests.rs) |
| Any other function in `SELECT` (`upper(name)`, `row_number() OVER ()`) | No | `-- column: <name> <RustType>`, or a cast | [`reports_unsupported_statement_shapes`](../../src/queries/analyze/tests.rs), [`unknown_functions_and_unsupported_return_types_still_fail`](../../src/queries/analyze/tests.rs) |
| Any function in `WHERE` or `HAVING`: not typed, and needn't be; its columns must exist. A placeholder inside one is typed by a cast (`lower($1::text)`) | Partly | A bare placeholder inside (`lower($1)`) fails with `-- param: $1 <name> <RustType>` | [`a_function_in_where_types_a_cast_or_declared_placeholder`](../../src/queries/analyze/tests.rs), [`a_type_free_override_needs_an_inferable_placeholder`](../../src/queries/analyze/tests.rs) |

## Statements and `FROM`

| SQL form | Typed | When it isn't | Test |
|---|---|---|---|
| CTEs (`WITH`): each is a table for the ones after it; a column list renames | Partly | A column the engine can't type fails asking for a cast (`-- column:` doesn't reach a CTE) | [`cte_results_join_like_tables_and_type_their_placeholders`](../../src/queries/analyze/tests.rs), [`later_ctes_see_earlier_ones_and_column_lists_rename`](../../src/queries/analyze/tests.rs), [`ctes_are_scoped_to_their_statement_and_must_be_typable`](../../src/queries/analyze/tests.rs) |
| `WITH RECURSIVE`: typed from the anchor | Yes | A recursive term with other columns or types fails naming them | [`recursive_ctes_take_their_columns_from_the_anchor`](../../src/queries/analyze/tests.rs), [`recursive_terms_must_match_the_anchor`](../../src/queries/analyze/tests.rs), [`partial_index_upserts_and_recursive_ctes`](../../tests/fixture-app/tests/postgres/queries.rs) |
| Data-modifying CTEs (`WITH x AS (UPDATE … RETURNING …)`, `INSERT`, `DELETE`), with any final statement | Yes | Only in the statement's own `WITH`, and not in a `:page` query; either fails saying so | [`data_modifying_ctes_feed_the_final_statement`](../../src/queries/analyze/tests.rs), [`any_final_statement_reads_a_data_modifying_cte`](../../src/queries/analyze/tests.rs), [`data_modifying_ctes_belong_to_the_top_level_with`](../../src/queries/analyze/tests.rs), [`data_modifying_ctes_run_as_one_statement`](../../tests/fixture-app/tests/postgres/queries.rs) |
| `UPDATE t SET … FROM a JOIN b …` and `DELETE FROM t USING a, b …`: typed like the join | Yes | An ambiguous unqualified column fails naming each table; `UPDATE t JOIN …` fails: join in `FROM` | [`update_from_resolves_columns_across_the_target_and_its_from_tables`](../../src/queries/analyze/tests.rs), [`delete_using_resolves_columns_like_a_join`](../../src/queries/analyze/tests.rs), [`update_from_names_the_tables_of_an_ambiguous_column`](../../src/queries/analyze/tests.rs) |
| `INSERT … SELECT`: each value typed from its target column | Yes | Too many or too few values, or `SELECT *`, fail naming the counts | [`insert_select_types_its_values_from_the_target_columns`](../../src/queries/analyze/tests.rs), [`insert_select_with_the_wrong_number_of_values_fails_naming_them`](../../src/queries/analyze/tests.rs) |
| `UNION [ALL]`, `INTERSECT`, `EXCEPT`, at the top level, in a CTE or a subquery | Yes | Branches with different column counts or unshared types fail naming the column | [`a_union_types_its_columns_from_every_branch`](../../src/queries/analyze/tests.rs), [`union_branches_must_match_column_by_column`](../../src/queries/analyze/tests.rs) |
| A subquery in `FROM`, `(SELECT …) AS x(a, b)`, in a `SELECT`, `UPDATE … FROM`, `DELETE … USING` or data-modifying CTE | Yes | It sees the `FROM` tables beside it only when `LATERAL`; otherwise a reference to one fails: ``write `LATERAL (SELECT …)` to refer to it`` | [`a_subquery_in_from_acts_as_a_table_typed_from_its_query`](../../src/queries/analyze/tests.rs), [`a_subquery_alias_column_list_renames_its_columns`](../../src/queries/analyze/tests.rs) |
| `LATERAL (SELECT …) x`: sees the outer row | Yes | | [`only_a_lateral_subquery_sees_the_tables_beside_it`](../../src/queries/analyze/tests.rs) |
| Set-returning functions in `FROM`: `generate_series`, `unnest`, `jsonb_to_recordset` / `json_to_recordset` / `jsonb_to_record` / `json_to_record` | Partly | Any other function in `FROM` fails naming the ones that are typed; `unnest($1)` needs an array cast, `jsonb_to_recordset` a column definition list, `generate_series` a cast bound (and a step for dates) | [`generate_series_types_its_column`](../../src/queries/analyze/tests.rs), [`unnest_types_its_array_parameter_and_column`](../../src/queries/analyze/tests.rs), [`jsonb_to_recordset_types_its_columns_and_json_parameter`](../../src/queries/analyze/tests.rs), [`set_returning_functions_explain_what_they_need`](../../src/queries/analyze/tests.rs), [`set_returning_functions_write_batches`](../../tests/fixture-app/tests/postgres/queries.rs) |
| `:page` queries: a `SELECT` with an `ORDER BY` and no `LIMIT`/`OFFSET`/`FETCH`; rows are always a `{Name}Row` | Yes | No `ORDER BY`, its own `LIMIT`/`OFFSET`/`FETCH`, or a non-`SELECT` fails naming it | [`a_page_query_is_analyzed_like_a_many`](../../src/queries/analyze/tests.rs), [`a_page_query_needs_an_order_by`](../../src/queries/analyze/tests.rs), [`a_page_query_with_its_own_limit_or_offset_fails_naming_it`](../../src/queries/analyze/tests.rs), [`page_queries_return_rows_and_the_total`](../../tests/fixture-app/tests/postgres/queries.rs) |
| More than one statement in a query | No | Call one function per statement on `&mut *tx` | [`reports_unsupported_statement_shapes`](../../src/queries/analyze/tests.rs) |

## Parameters

| SQL form | Typed | When it isn't | Test |
|---|---|---|---|
| Comparison, `IN`, `BETWEEN` (`<column>_from`/`_to`), `LIKE` against a column; `= ANY($n)` (a slice); `LIMIT`/`OFFSET` (`i64`) | Yes | | [`infers_params_from_comparisons_and_limits`](../../src/queries/analyze/tests.rs), [`infers_params_from_in_any_between_like_and_casts`](../../src/queries/analyze/tests.rs), [`between_bounds_are_named_from_and_to`](../../src/queries/analyze/tests.rs) |
| `INSERT … VALUES`, `INSERT … SELECT` values, `UPDATE … SET`, `ON CONFLICT … DO UPDATE SET` | Yes | | [`insert_values_take_column_types_and_nullability`](../../src/queries/analyze/tests.rs), [`update_set_and_upsert_assignments_are_typed`](../../src/queries/analyze/tests.rs) |
| Arithmetic with a numeric column (`rooms_held + $2`) | Partly | A timestamp plus `$n` isn't settled (it's an interval): `-- param:` or a cast | [`arithmetic_with_a_numeric_column_types_its_placeholder`](../../src/queries/analyze/tests.rs) |
| `coalesce($1, column)`: the column's type, as `Option` | Partly | `coalesce($1, $2)` or `coalesce($1, 'x')` fails with `-- param:` | [`coalesce_types_a_placeholder_from_the_column_beside_it`](../../src/queries/analyze/tests.rs), [`coalesce_without_a_column_still_asks_for_a_param_override`](../../src/queries/analyze/tests.rs) |
| A cast (`$1::int`, `$1::product_status`, `$1::uuid[]`) | Yes | | [`infers_params_from_in_any_between_like_and_casts`](../../src/queries/analyze/tests.rs), [`casts_to_enum_types_infer_the_generated_enum`](../../src/queries/analyze/tests.rs) |
| Boolean switches: `$2::bool`, `CASE WHEN $2 THEN … END` and `NOT $2` (`bool`) in `WHERE` and `ORDER BY`, one `CASE` per sort direction | Yes | | [`a_bool_cast_placeholder_switches_where_and_order_by`](../../src/queries/analyze/tests.rs), [`a_bool_cast_placeholder_switches_a_count`](../../src/queries/analyze/tests.rs), [`a_switch_in_order_by_alone_is_typed`](../../src/queries/analyze/tests.rs), [`one_query_lists_both_sides_of_a_moment`](../examples/api/tests/service.rs) |
| A placeholder in a subquery (scalar, `EXISTS`, `IN`): typed from the subquery's own tables first | Yes | | [`subqueries_resolve_their_own_tables_first`](../../src/queries/analyze/tests.rs) |
| A placeholder anywhere else (a function argument, a bare select item) | No | ``cannot infer the type of `$1` from its context; add `-- param: $1 <name> <RustType>` `` | [`reports_untypable_and_unused_placeholders`](../../src/queries/analyze/tests.rs) |
| An unused placeholder below a used one, or one used as two types | No | Fails naming both; renumber, or cast one use | [`reports_untypable_and_unused_placeholders`](../../src/queries/analyze/tests.rs) |

## Annotations

| Annotation | What it does | Test |
|---|---|---|
| `-- param: $2 since` / `since!` / `since Uuid` | Renames and keeps the inferred type / drops a nullable column's `Option` / takes the written type. A name without a type needs a placeholder the engine can type | [`a_param_override_without_a_type_renames_and_keeps_the_inferred_type`](../../src/queries/analyze/tests.rs), [`a_bang_marks_a_nullable_columns_param_not_null`](../../src/queries/analyze/tests.rs), [`a_type_free_override_needs_an_inferable_placeholder`](../../src/queries/analyze/tests.rs), [`overrides_supply_types_inference_cannot`](../../src/queries/analyze/tests.rs) |
| `-- column: total i64` | Settles a result column's type and nullability; may name a typed id or enum | [`overrides_supply_types_inference_cannot`](../../src/queries/analyze/tests.rs), [`a_column_override_names_generated_typed_ids_and_enums`](../../src/queries/analyze/tests.rs) |
| `-- rok:skip [<note>]` | Leaves the query ungenerated, not failed | [`a_rok_skip_query_is_neither_analyzed_nor_failed`](../../src/queries/analyze/tests.rs) |
| `-- returns: FindProductRow` / `Category` / `crate::path::Type` | Shares another query's row type or a model's (a column that differs in name, order, type or nullability fails naming both types), or decodes into a hand-written `sqlx::FromRow` type, unchecked by the engine | [`returns_reuses_a_row_type_of_the_file`](../../src/queries/analyze/tests.rs), [`returns_names_a_model_or_a_hand_written_type`](../../src/queries/analyze/tests.rs), [`a_query_decodes_into_a_hand_written_type`](../examples/api/tests/service.rs) |
| `-- rok:params struct` / `positional` | Fixes the function's parameter shape over the project's `query_params`; the doc comment names the rule | [`rok_params_fixes_the_shape`](../../src/queries/annotations.rs), [`the_parameter_shape_follows_the_annotation_then_the_project`](../../src/generator/queries.rs) |
