# 02: `rok schema column add`

Spec: `docs/specs/schema-editing.md`

**What to build:** `rok schema column add <table>.<column> <TYPE>` with `--null`/`--not-null`, `--default`, `--unique`, `--references` (`--on-delete`), `--check` and `--after`.
- **The edit:** it finds the one `CREATE TABLE` defining the table in the schema dir and adds one line (default: before the timestamp columns, else last), keeping every other byte.
- **After it:** it prints the next `rok gen --migration` with a suggested name; `--migration [NAME]` stages it; `--dry-run` prints the diff and writes nothing.
- **Shared ground:** this ticket lays the shared editing core (locate the table, splice a line, keep bytes) the other column commands use.

**Blocked by:** starter-feedback/24 (one rule for finding the project)

**Status:** blocked

- [ ] The edited file equals the expected one byte for byte, LF and CRLF cases
- [ ] A table in no file, or in two, is an error naming them
- [ ] `--migration` stages an `ADD COLUMN`; `--dry-run` writes nothing
- [ ] `-j` payload works for it
