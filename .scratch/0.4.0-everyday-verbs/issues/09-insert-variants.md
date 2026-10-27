# 09: Insert variants — insert_get_id, insert_using

**What to build:**
1. **`Insert::insert_get_id(executor) -> Result<PK>`** — inserts and returns the generated primary key. For PG with `RETURNING`, returns the PK from the returned row. Useful when the PK is not in the model struct (e.g., `SERIAL`/`IDENTITY` not in Rust fields).
2. **`Insert::insert_using(subquery: Select<Other>) -> Insert<Self>`** — bulk insert from a SELECT: `INSERT INTO target (...) SELECT ... FROM ...`. The subquery's projection must match the insert columns.

Both are low-level `Insert` builder methods (not model statics), available for hand-written and generated code.

**Blocked by:** 02 (Insert builder methods), 05 (casting for subquery compatibility).

**Status:** ready-for-agent

- [ ] `Insert::insert_get_id` in `crates/rok-data-core/src/query/insert.rs`
- [ ] `Insert::insert_using` taking a `Select<Other>` with matching projections
- [ ] Facade tests: returning PK, subquery column matching
- [ ] Module fixture expectations (if generator uses them)