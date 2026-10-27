# 09: Insert variants — insert_get_id, insert_using

**What to build:**
1. **`Insert::insert_get_id(executor) -> Result<PK>`** — inserts and returns the generated primary key. For PG with `RETURNING`, returns the PK from the returned row. Useful when the PK is not in the model struct (e.g., `SERIAL`/`IDENTITY` not in Rust fields).
2. **`Insert::insert_using(subquery: Select<Other>) -> Insert<Self>`** — bulk insert from a SELECT: `INSERT INTO target (...) SELECT ... FROM ...`. The subquery's projection must match the insert columns.

Both are low-level `Insert` builder methods (not model statics), available for hand-written and generated code.

**Blocked by:** 02 (Insert builder methods), 05 (casting for subquery compatibility).

**Status:** done (2026-10-27) — one item dropped by decision, below

- [x] `insert_get_id` — **dropped by decision:** a model's primary key is always one of its fields (its columns are its fields), so `Tag::insert_builder()….exec(&db).await?.id` already returns it with the stored row; the case the spec names (a key outside the struct) can't occur
- [x] `insert_using` → **`select.insert_into::<Target>([columns])`** on a projected query, read in SQL order: `Post::filter(…).select((Post::TITLE,)).insert_into::<Archive>([Archive::TITLE])`, with `exec` (row count) and `returning` (the inserted rows); a column count different from the selection's is `Error::InvalidQuery`
- [x] Facade tests: `tests/features/lifecycle.rs` (SQL shape and database)
- [x] Module fixture expectations — not applicable: the generator doesn't use them
