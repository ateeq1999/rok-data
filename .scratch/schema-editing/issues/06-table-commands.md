# 06: `rok schema table add|remove|rename`

Spec: `docs/specs/schema-editing.md`

**What to build:** the table counterparts of the column commands, on the same editing core.
- **`add`:** writes a new `CREATE TABLE` with the project's conventions (primary key, timestamps), in a new schema file or the one `--file` names.
- **`remove`:** refuses a table other tables reference, unless `--force`.
- **`rename`:** records itself for the next migration, like a column rename.

**Blocked by:** 05

**Status:** blocked

- [ ] Each command tested by its output files
- [ ] A table rename stages `ALTER TABLE … RENAME TO`
