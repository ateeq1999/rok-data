# 01: Errors as paths, and the constraint backstop

**What to build:** `FieldError.field` becomes an owned dotted path (`address.city`, `items.2.sku`) shared by models and inputs, with optional rule `params`. A unique (23505) or foreign-key (23503) violation on a model write becomes `Error::Validation` with the field named by PostgreSQL's detail and code `unique` / `exists`, when the columns map to fields; `is_unique_violation()` / `is_foreign_key_violation()` stay true.

**Blocked by:** None

**Status:** done (2026-10-27) — the mapping covers the row write verbs; the builders keep the database error (documented)

- [x] `FieldError { field: String, code, message, params }`; model derive and engine call sites updated
- [x] Violation mapping on insert/update/upsert, multi-column keys reported on each column
- [x] Facade tests (DB-backed): duplicate email → `Error::Validation` at `email`, code `unique`; FK miss → `exists`
- [x] Migration guide + CHANGELOG (breaking)
