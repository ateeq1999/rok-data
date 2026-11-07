# 49: Set-returning functions in queries

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** query typing understands `generate_series(...)` and `unnest(...)` in `FROM` from their known signatures, so a copy-forward like `INSERT … SELECT … FROM generate_series($1::date, $2::date, '7 days') AS week` types. `jsonb_to_recordset($1) AS x(col type, …)` (and `json_to_recordset`) is typed from its column definition list, with `$1` a `serde_json::Value`, so a query can insert many lines from one JSON parameter. The queries guide documents the batch patterns `unnest($1::date[])` and `jsonb_to_recordset` with tests.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `generate_series` over dates and integers types its column
- [x] `unnest($1::uuid[])` types its parameter as `Vec<Uuid>` / `&[Uuid]`
- [x] `INSERT INTO order_items (…) SELECT … FROM jsonb_to_recordset($2) AS l(item_id uuid, quantity int)` types its columns and `$2`
- [x] Guide example compiles (doc test or fixture)
