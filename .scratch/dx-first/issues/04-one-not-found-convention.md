# 04: One "not found" convention across every verb

**What to build:** A user learns one rule: a verb that targets one row by key returns the row (or `()`), and a missing row is `Error::NotFound`. Today the model's `update`/`delete` raise `NotFound`, while the generated `patch` returns `Result<Option<Book>>` and `destroy` returns `Result<u64>`. Optional lookups stay explicit (`find` → `Option`, `find_or_fail` → `NotFound`).

**Blocked by:** 03

**Status:** done

- [x] Generated `patch` → `Result<Book>`, `destroy` → `Result<()>`, both `NotFound` on a missing key
- [x] Generated handlers map `NotFound` to the wire standard's 404 without per-verb `Option` handling
- [x] CHANGELOG breaking-changes entry with before/after call sites
- [x] Facade + fixture tests cover the missing-row path for each verb (DB-backed: they skip with a printed note when `DATABASE_URL` is unset)
