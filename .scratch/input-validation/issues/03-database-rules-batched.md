# 03: Database rules, batched

**What to build:** `unique(Model::COL, except = field, case_insensitive, scope = path)` and `exists(Model::COL, scope = path)` on input fields, the target model's scopes applying; run only for fields whose field rules passed; all of an input's built-in database rules in one `SELECT EXISTS(…) AS …` statement. Custom `database = path` (field) and `validate_with_database = path` (record) after them. A database rule on `#[derive(Model)]` fails to compile naming `Input`.

**Blocked by:** 02

**Status:** done (2026-10-27)

- [x] Batched statement (SQL-shape test) and DB-backed tests for each option
- [x] `except` naming a missing field is a compile error
- [x] Validating inside a transaction (test)
