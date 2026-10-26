# 04: Schema-reading fixes (rok-sqlc 0.8)

**What to build:** pg_dump-shaped schemas generate correctly: `ALTER TABLE … DROP CONSTRAINT [IF EXISTS]` is followed; dropping an unknown constraint warns instead of failing the table; `CHECK (col = ANY (ARRAY[...]))` is read as a value list; `RENAME CONSTRAINT` checks both names; `IS NULL` matches in any case.

**Blocked by:** dx-first 01

**Status:** done

- [x] Each case covered in the engine's migration-alters library suite
- [x] Warning text printed, never silent

**How it shipped:**
- **Upstream's patch:** applied as `git diff v0.5.0 v0.8.0 -- src/parser src/schema src/queries`. The new modules applied cleanly: `named.rs`, `names.rs`, `schema/constraints.rs`, and the parser tests in `tests/constraint_changes.rs`.
- **Rejected hunks, merged by hand:**
  - `alter.rs` and `table.rs`: the column-level checks and foreign keys now go through `add_check` and `add_foreign_key`.
  - `constraints.rs`: `DeclaredForeignKey`.
  - `schema/mod.rs` and `types.rs`: the constraint types moved to `schema/constraints.rs`.
- **Workspace additions kept:** `ForeignKey::{on_delete, on_update}`. The workspace's own inline-FK naming loop is dropped, because `add_foreign_key` names unnamed keys.
- **Warnings reach the user:** `generate.rs` appends `parsed.warnings` to the report, as upstream does. Before, the workspace collected them and dropped them, so the "warns, never silent" checkbox needed this.
- **Wording:** the warning text says `rok sqlc`.
- **Tests:**
  - parser unit tests (`constraint_changes`, in the engine crate) cover every case: DROP of each kind, IF EXISTS, untracked names, `= ANY (ARRAY[...])`, RENAME checks, and `IS NULL` in any case;
  - rok-cli's `tests/cli/constraint_changes.rs` (binary, 4) proves the enum widens, lookups and relationships drop, and the warning prints at its `path:line`.
- **No generated output changed** in any fixture, template or snapshot.
