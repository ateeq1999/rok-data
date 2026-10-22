# 02: Query builder sugar — where_any/all/none, JSON, date-parts, column compare, union, raw, tap/pipe, debug

**What to build:** Chain methods on `Select<M>` for common Laravel query patterns: `where_any(columns, op, value)`, `where_all`, `where_none`; `where_json_contains(col, path, value)`, `where_json_contains_key(col, path)`, `where_json_length(col, path, op, n)`; `where_date/month/day/year/time(col, value)`; `where_column(col1, op, col2)`, `where_null_safe_eq(col, value)`; `union(other)`, `union_all(other)`; `increment_each(&[(Col, Value)])`; `tap(f)`, `pipe(f)`; `debug()`, `debug_raw()`; `select_raw`, `where_raw`, `having_raw`, `order_by_raw`, `group_by_raw`. All return `Self` for chaining.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] `Select::where_any/where_all/where_none` using `Expr::any_of`/`all_of`/`!any_of`
- [ ] JSON methods wrapping existing `Column::json_contains`/`json_has_key`/`json_path_text` + length
- [ ] Date-part methods using `DATE(col) = ...` / `EXTRACT(... FROM col) = ...`
- [ ] `where_column` emitting `col1 OP col2`; `where_null_safe_eq` emitting `(col = val OR (col IS NULL AND val IS NULL))`
- [ ] `union` / `union_all` requiring compatible projections
- [ ] `increment_each` building multiple `SET col = col + val` in one UPDATE
- [ ] `tap(self, f: impl FnOnce(&mut Self)) -> Self`, `pipe(self, f: impl FnOnce(Self) -> T) -> T`
- [ ] `debug() -> Self` prints `to_sql()` behind `cfg(debug_assertions)`; `debug_raw() -> Self` prints interpolated SQL
- [ ] Raw methods wrapping `Raw`/`Expr::raw`
- [ ] Facade tests: SQL-shape assertions for each method
- [ ] Module fixture expectations for generated query files