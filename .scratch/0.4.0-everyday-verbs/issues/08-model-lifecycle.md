# 08: Model lifecycle — first_or_create, update_or_create, fresh, replicate, increment/decrement/touch

**What to build:**
1. **`Model::first_or_create(executor, attrs, values?) -> Result<Self>`** — atomic find-or-insert: `SELECT ... FOR UPDATE` then `INSERT ... ON CONFLICT DO NOTHING RETURNING *` (PG). `attrs` = unique columns to match; `values` = additional columns for insert.
2. **`Model::update_or_create(executor, attrs, values) -> Result<Self>`** — atomic upsert: `ON CONFLICT (unique_cols) DO UPDATE SET ... RETURNING *`.
3. **`model.fresh(executor) -> Result<Self>`** — new instance from DB by PK (already have `reload` which mutates self; `fresh` returns new).
4. **`model.replicate(exclude: &[Column<Self>]) -> Self`** — unsaved copy with excluded columns reset to `Default::default()`.
5. **`model.increment(col, by)`, `decrement(col, by)`, `touch()`** — `UPDATE ... SET col = col + by, updated_at = now()` / `updated_at = now()`. `touch` only updates `updated_at`.

Generator emits these on generated models. Hand-written models get them via `Model` trait.

**Blocked by:** 02 (increment uses Update chain), 05 (casting for increment value types).

**Status:** ready-for-agent

- [ ] `Model` trait additions: `first_or_create`, `update_or_create`, `fresh`, `replicate`, `increment`, `decrement`, `touch`
- [ ] Implementation uses `ON CONFLICT` for atomicity (PG-specific, documented)
- [ ] `replicate` uses `value_of` for all columns except excluded; excluded get `Default::default()`
- [ ] `touch` only works if model has `updated_at` column
- [ ] Generator emits on generated models
- [ ] Facade tests: atomicity, exclude behavior, touch updates timestamp
- [ ] Module fixture expectations