# 02: Query builder sugar — where_any/all/none, JSON, date-parts, column compare, union, raw, tap/pipe, debug

**What to build:** Chain methods on `Select<M>` for common query patterns: `where_any(columns, op, value)`, `where_all`, `where_none`; `where_json_contains(col, path, value)`, `where_json_contains_key(col, path)`, `where_json_length(col, path, op, n)`; `where_date/month/day/year/time(col, value)`; `where_column(col1, op, col2)`, `where_null_safe_eq(col, value)`; `union(other)`, `union_all(other)`; `increment_each(&[(Col, Value)])`; `tap(f)`, `pipe(f)`; `debug()`, `debug_raw()`; `select_raw`, `where_raw`, `having_raw`, `order_by_raw`, `group_by_raw`. All return `Self` for chaining.

**Blocked by:** None (can start immediately).

**Status:** done (2026-10-27) — shaped by decision, below

**Decision — typed conditions on the column, not `(column, op, value)` strings.** A string operator loses the column's type and the compiler's help, and a `where_date(col, …)` on `Select` duplicates what a composable condition already does. So the conditions live on `Column` and compose with `filter` / `and` / `or` like every other one; `Select` gains sugar only where it reads better than a condition (several columns at once, raw escapes, composing, `UNION`). The names follow the chain's canonical verb, `filter` (`filter_any`, `filter_raw`), as `filter_if` / `filter_opt` do.

- [x] `Select::filter_any/filter_all/filter_none(columns, |c| condition)` (was `where_any/all/none`)
- [x] JSON: `Column::json_array_len()`, `json_array_len_at(path)`, `json_has_path(path)`, `json_contains_at(path, json)`, beside the existing `json_contains` / `json_has_key` / `json_text`
- [x] Date parts: `Column::date()`, `time()`, `year()`, `month()`, `day()`, `hour()` as projections (`CAST` / `EXTRACT … AS INTEGER`), compared like any value
- [x] `Column::eq_column` … `lte_column` (`col1 OP col2`); `is_not_distinct_from` / `is_distinct_from` (PostgreSQL's own `NULL`-aware equality, instead of an `OR … IS NULL` pair)
- [x] `Select::union` / `union_all` → a `Union<M>` with `order_by`, `limit`, `offset`, `all` / `get` / `first`; each leg keeps its own filters, scopes and limit
- [x] `Update::increment_each([(column, by)])` in one `UPDATE` (`Update::increment` / `decrement` already existed)
- [x] `pipe(f: FnOnce(Self) -> T) -> T`; `tap(f: FnOnce(&Self)) -> Self` (looks, doesn't change: `scope` already applies a `Self -> Self` step)
- [x] `debug()` prints the statement and its parameters to stderr, like `dbg!`. **`debug_raw` dropped by decision:** inlining bound values gives a string that looks runnable but loses the parameters' types and escaping; `to_sql()` gives both parts
- [x] `filter_raw`, `having_raw`, `order_by_raw`, `group_by_raw` (`GROUP BY` now holds any term), `select_raw`
- [x] Facade tests: SQL-shape for each, and DB-backed for the date parts, JSON, column comparisons, `NULL`-aware equality, `UNION` and `increment_each` (`tests/features/query_sugar.rs`)
- [x] Module fixture expectations — not applicable: the generator emits none of these
