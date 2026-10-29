---
name: rok-data-migrations
description: >
  Migration workflow for rok-data: creating, staging, applying, reverting
  and diagnosing `NNNN_name.up.sql`/`.down.sql` pairs, the
  `_rok_data_migrations` journal, checksum drift and the
  applied|pending|edited|unknown states, `rok migrate
  create|status|up|down` with the `--json` StatusRow contract, staging a
  schema delta with `rok gen --migration` (`--allow-destructive`,
  `--rename`), and CI freshness rules. Fires for "migration", "drift",
  "journal", "staging", "database is out of sync", migrations failing,
  or CI red on drift in rok-data projects.
---

## File format (what runs)

One directory — default `migrations/` — holding pairs:

```
0001_init.up.sql          ┐ version + name, required: NNNN_name
0001_init.down.sql        ┘
0002_add_slug.up.sql      (the .down.sql side is optional)
```

- Only `NNNN_name.up.sql` / `NNNN_name.down.sql` files are loadable; any
  other file name is a load *error* (hidden files are skipped — the
  staging's `.rok-schema.json` snapshot lives beside the pairs).
- `rok migrate create NAME` writes an empty pair (`{version:04}_NAME`,
  next free version) with skeleton comments: the `.up.sql` gets
  `-- down`-side guidance, the `.down.sql` `-- undo NAME`.

## Applying

```sh
export DATABASE_URL=postgres://…
rok migrate status            # the journal state per file
rok migrate up                # apply pending, oldest first
rok migrate down              # revert the newest applied (alias: rollback)
```

Engine guarantees (do not re-implement or bypass): one transaction per
migration, a PostgreSQL advisory lock so two processes never race, and
refusal to run when an applied migration's `up` SQL **checksum drifted**
(the `edited` state — fatal; checksum = FNV-1a of the `up` text in the
journal `_rok_data_migrations`), when the database holds a version the
files don't know (`unknown`), or when a pending migration is older than
the newest applied.

**Editing an applied pair is always wrong**: stage a new pair instead.
`--json` exists *because* consumers need the drift fact without crashing
the workflow: `status|up|down --json` = one JSON document on stdout,
`{"migrations": [{"version","name","state","checksum","drift"}…]}` with
state ∈ `applied|pending|edited|unknown`.

## Staging from the schema (the front door)

`rok gen --migration NAME` diffs the schema dir against the last staged
pair (the snapshot beside them) and writes the pair — prohibited steps
(destructive columns/tables) fail until `--allow-destructive` is
explicit; `--rename tbl=…` / `--rename tbl.col=…` preserves identity as
rename instead of drop+create. This staging is also available inside the
legacy whole-crate generator as `rok generate --migration`.

## CI contract

`rok gen check` — writes nothing; missing/changed/stale files reported;
**exit 1** on drift. A red gate means: regenerate (`rok gen`), stage the
delta (`--migration`) or accept `--force` only for deliberate template
moves. The `--json` shapes and every exit code live in
[`docs/cli-reference.md`](../../../docs/cli-reference.md).

## Single-file sections

The rok family's `-- UP` / `-- DOWN` single-file convention is this
pair-format's ancestor (see
[`docs/migration-guide.md`](../../../docs/migration-guide.md) §From
rok-fluent and workspace ADR 0004 for the unification story); the pair
format is what `migration_file::load` reads byte-for-byte today, and
generated crates embed the same text with `include_str!`, so checksums
match constants across sources.

## Debug moves

- `status --json` first: the state names the failure (`edited` → compare
  the file with what the journal's checksum remembers; `unknown` → the
  DB ran a migration the tree deleted or renumbered).
- `rok db check --json` before blaming migrations: many "migration
  failures" are unreachable databases. `SERVER` version and connection
  details print masked.
- Renumbering deployed pairs is never safe: versions are identity in
  the journal.
