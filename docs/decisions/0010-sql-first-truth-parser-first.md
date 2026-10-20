# ADR 0010: SQL-first source of truth, parser-first with DB verification

Date: 2026-10-19 · Status: accepted

## Context

The product asks for two directions at once: write `schema.sql` (or a
folder of `.sql` files) and get a running API, and "from code, get
`.sql` files back". Literal bidirectional sync — or authoring DDL from
parsed Rust structs — is unbounded machinery, and today's engines
disagree on truth: codegen diffs an offline `schema.json` snapshot into
migration pairs, while the sqlc engine parses schema text only.

## Decision

- The **schema dir** is the single writable source of truth; a
  project's `migrations/` directory remains equally valid schema input
  (preserving the current engine behavior).

- `rok gen` diff-writes versioned migration pairs by extending the
  codegen path's offline `ddl::diff` over the `schema.json` snapshot:
  `--migration NAME` staging, `--allow-destructive`, and
  `--rename table.old=new` semantics are preserved. ADR 0004's journal
  and file formats stay untouched; hand-written migrations remain the
  escape hatch the diff engine flags against.

- The reverse direction (code → `.sql`) is deterministic
  reconciliation, never a second author of truth: generated artifacts
  carry their canonical SQL lineage (`schema.json`, generated migration
  pairs, `rok schema dump`), and drift is checkable
  (`rok gen check`) so CI fails when code and schema part ways.

- Generation is parser-first — instant, offline, bounded DDL subset.
  When a database is configured, catalog verification reconciles column
  types against the built-in parser with a printed warning; `--strict`
  turns warnings into failures. With no database configured,
  parser-only generation still succeeds.

Rejected: exporting DDL from arbitrary hand-written `#[derive(Model)]`
structs (a second truth-writer whose only real payoff — catching drift —
the check already provides), and live-database-only generation (no
offline DX).

## Consequences

- `schema.json` becomes the shared round-trip artifact of the whole
  pipeline; both engines read and write the same shape.
- Parser gaps become first-class UX: an unsupported DDL construct is
  exactly the case the DB-verified pass settles.
