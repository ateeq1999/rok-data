# 08: Model lifecycle — first_or_create, update_or_create, fresh, replicate, increment/decrement/touch

**What to build:**
1. **`Model::first_or_create(executor, attrs, values?) -> Result<Self>`** — atomic find-or-insert: `SELECT ... FOR UPDATE` then `INSERT ... ON CONFLICT DO NOTHING RETURNING *` (PG). `attrs` = unique columns to match; `values` = additional columns for insert.
2. **`Model::update_or_create(executor, attrs, values) -> Result<Self>`** — atomic upsert: `ON CONFLICT (unique_cols) DO UPDATE SET ... RETURNING *`.
3. **`model.fresh(executor) -> Result<Self>`** — new instance from DB by PK (already have `reload` which mutates self; `fresh` returns new).
4. **`model.replicate(exclude: &[Column<Self>]) -> Self`** — unsaved copy with excluded columns reset to `Default::default()`.
5. **`model.increment(col, by)`, `decrement(col, by)`, `touch()`** — `UPDATE ... SET col = col + by, updated_at = now()` / `updated_at = now()`. `touch` only updates `updated_at`.

Generator emits these on generated models. Hand-written models get them via `Model` trait.

**Blocked by:** 02 (increment uses Update chain), 05 (casting for increment value types).

**Status:** done (2026-10-27) — three items resolved by existing verbs, by decision, below

- [x] `first_or_create` → **`row.first_or_insert(&db, [Tag::SLUG])`**: the row itself carries the values (typed, no attribute map), the columns name the unique key. One statement (`WITH … INSERT … ON CONFLICT DO NOTHING RETURNING` `UNION ALL` the existing row), so it is atomic without a lock; `after_insert` fires only when it inserted
- [x] `update_or_create` — **already exists** as `row.upsert_on(&db, [columns])`; no second name
- [x] `fresh` — **already exists** as `row.reload(&db)`, which returns a new row (it never mutated `self`); no second name
- [x] `replicate` — **dropped by decision:** `row.insert(&db)` already writes a copy, since inserts skip generated keys and managed timestamps; resetting other fields is a struct update (`Row { title: …, ..row }`), which the compiler checks
- [x] `increment(&db, col, by)`, `decrement`, `touch(&db)` on `ModelWrite`: `UPDATE … SET col = col + by` without reading first, `updated_at` / version maintained, the stored row returned; `touch` without an `updated_at` column is `Error::InvalidQuery`
- [x] Generated models get them through the derive's `ModelWrite` impl
- [x] Facade tests: `tests/features/lifecycle.rs`
- [x] Module fixture expectations — not applicable: nothing new is emitted
