# 03: `exists` and `unique` pre-checks (rok-sqlc 0.5)

**What to build:** A create or update whose foreign key names a missing row, or whose value collides with a unique index, answers `422` naming the field instead of a bare `409`/`500`. Unique indexes on `lower(col)` are treated as case-insensitive.

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] Pre-checks generated for FK and unique columns
- [ ] `lower(col)` unique index detection
- [ ] Template integration tests for both 422 paths (DB-backed, printed skip without a URL)
- [ ] CHANGELOG breaking entry (409 → 422)
