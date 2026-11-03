# 04: `rok schema column remove`

Spec: `docs/specs/schema-editing.md`

**What to build:** `remove <table>.<column>` deletes the column's line.
- **Refusal:** it refuses a column an index, a foreign key, a `CHECK`, a query in `queries/` or a per-column API config list uses, naming each use; `--force` removes it anyway.
- **Config:** it updates the config's per-column lists.
- **Migration:** the staged `DROP COLUMN` carries the data-loss warning.

**Blocked by:** 02

**Status:** blocked

- [ ] Each kind of use refuses with its name and location
- [ ] `--force` removes the column and the config entries
