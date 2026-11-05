# 30: `SCHEMA_SQL` in dependency order

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the engine emits `src/db/schema.rs`'s `SCHEMA_SQL` with its statements in dependency order, the order the migration diff uses, instead of file-name order. A table referencing a table defined in a later schema file (`users.home_city_id` in 0001 → `cities` in 0007) loads cleanly. The doc comment says the tests load it.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A forward reference across schema files loads in a DB test (red first)
- [ ] Order for already-ordered schemas is unchanged, or the snapshot diffs are reviewed
- [ ] Changelog updated
