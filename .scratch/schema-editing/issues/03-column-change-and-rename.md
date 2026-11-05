# 03: `rok schema column change` and `rename`

Spec: `docs/specs/schema-editing.md`

**What to build:**
- **`change`:** `change <table>.<column>` with `--type` and `add`'s options rewrites only what it's given on that column's line.
- **`rename`:** `rename <table>.<old>=<new>` edits the line and records the rename as pending in the staging state. The next `rok gen --migration` applies it as `--rename` would and says so.
- **Config:** `rename` updates the column's entries in the App's `rok-data-sqlc.toml`.

**Blocked by:** 02

**Status:** blocked

- [ ] `change` keeps the parts of the column it wasn't given
- [ ] `rename` then `rok gen --migration` stages `RENAME COLUMN`, not drop + add
- [ ] The config's per-column lists follow the rename
