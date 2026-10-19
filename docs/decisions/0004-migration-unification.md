# ADR 0004: one migration engine, two accepted file formats

Date: 2026-10-19 · Status: accepted

## Context

rok-db and rok-fluent both had migration engines with different file
formats, journaling and status rules. Users of both exist; silently
picking one format would strand the other side.

## Decision

- The engine is `rok-data-core::migration` (rok-db's, the most

  developed: checksum-drift detection, edited/pending/unknown states,
  apply/revert with steps).

- The file source is the new `rok-data-core::migration_file`: it loads
  `<dir>/<NNNN_name>.up.sql` + `.down.sql` pairs (rok-db convention)
  **and** rok-fluent's single-file format with `-- up` / `-- down`
  sections.

- The file **name** `<version>_<name>` is required in both cases (this
  is the one breaking change against rok-fluent, which allowed any
  name); the status rows, checksums and drift semantics are identical
  for either format.

- Integration fixes applied while porting: version scanning once
  aborted when any non-`.sql` sibling file appeared next to a migration;
  the loader now skips non-SQL neighbors instead.

## Consequences

- `rok migrate create` writes file pairs; a rok-fluent-style project can

  keep editing its existing single-file migrations without conversion.

- `rok migrate status --json` prints the same `StatusRow` array with

  fields `version`, `name`, `state`, `checksum`, `drift` regardless of
  the source format — one machine contract.

- Journal table: `_rok_data_migrations` (renamed from rok-db's
  `_rok_db_migrations`, which the engine hard-codes; rok-db projects
  rename their table once, or `up` starts a fresh journal and re-runs
  already-applied SQL until it fails on existing objects). The `unknown`
  state remains the honest report for journal rows whose checksum has
  drifted or whose file is gone.
