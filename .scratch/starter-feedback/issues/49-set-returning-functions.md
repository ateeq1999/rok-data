# 49: Set-returning functions in queries

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** query typing understands `generate_series(...)` and `unnest(...)` in `FROM` from their known signatures, so a copy-forward like `INSERT … SELECT … FROM generate_series($1::date, $2::date, '7 days') AS week` types. The queries guide documents the batch pattern `unnest($1::date[])` with a test.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `generate_series` over dates and integers types its column
- [ ] `unnest($1::uuid[])` types its parameter as `Vec<Uuid>` / `&[Uuid]`
- [ ] Guide example compiles (doc test or fixture)
