# 03: `exists` and `unique` pre-checks (rok-sqlc 0.5)

**What to build:** A create or update whose foreign key names a missing row, or whose value collides with a unique index, answers `422` naming the field instead of a bare `409`/`500`. Unique indexes on `lower(col)` are treated as case-insensitive.

**Blocked by:** 02

**Status:** done

- [x] Pre-checks generated for FK and unique columns
- [x] `lower(col)` unique index detection
- [x] Template integration tests for both 422 paths (DB-backed, printed skip without a URL)
- [x] CHANGELOG breaking entry (409 → 422)

**How it shipped:**
- **Schema reading:** applied as upstream's v0.4.0→v0.5.0 patch, which applied cleanly. `UniqueKey::lowered`, `lookup_unique_keys`, and `lower(col)` in `CREATE UNIQUE INDEX`.
- **Lookups:** `get_by_<col>` on a case-insensitive key is a model chain with `rok_data::Expr::raw("lower(col) = lower(?)", …)`. The runtime stays untouched (spec).
- **The checks:** `prechecks.rs` and `unique.rs` copied from v0.5.0. `Endpoint` moved to `endpoint.rs` with `Bodies`, which also splits `feature.rs` (590 → 553 lines). `Feature` gains `references`, `unique_checks`, `knows_caller`, `takes_caller`, `tenant_of` and `caller_of`. The service's `_in` runs the checks after the owner lock, and handlers and docs take the per-endpoint caller and tenant. `listing.rs` gains `exists_owned` and `<columns>_taken`.
- **Tests (DB-backed, printed skip without a URL):**
  - the api-app's `0006_comments`, `0007_handles` and `0008_scoped_checks`, with upstream's `tests/references.rs` and `tests/unique.rs`;
  - the notes/shapes, `examples/api` and CLI `keys` tests updated, 409 → 422, as upstream did;
  - a read unit test for the lowered lookup.
- **Template tests:** these paths are covered by the api-app and `examples/api`. The templates have no foreign key into a generated body, and their unique email is immutable.
- **Docs:** the CHANGELOG breaking entries (engine and root), the configuring guide's "Validation" and "Pre-checks" sections, the README, and the architecture guide's quoted `create_in`. Engine ADR 0019's note is corrected.
