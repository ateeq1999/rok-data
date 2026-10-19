# Getting started with rok-data

This walks through the two typical projects: a rok-db-style application
(runtime + whole-crate generation) and a resource-module application
(engine generation). Prerequisites: Rust ≥ 1.94, PostgreSQL 12+.

## 1. The runtime, by hand

```console
$ cargo new notes && cd notes && cargo add rok-data
```

Point at a database (read first from `--database-url`, then the
`DATABASE_URL` environment variable):

```console
$ export DATABASE_URL=postgres://postgres:postgres@localhost:5432/notes
```

Write the first migration — `migrations/0001_create_notes.up.sql`:

```sql
CREATE TABLE notes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    text TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

… and `migrations/0001_create_notes.down.sql`:

```sql
DROP TABLE notes;
```

Check what the tool sees, then apply:

```console
$ rok migrate status
$ rok migrate up
```

`rok migrate status` prints the journal (`_rok_data_migrations`) state
per file; `--json` turns the same information into a machine contract
(see `docs/cli-reference.md`). `rok db check` verifies connectivity and
reports the server version; `rok db inspect --table notes` prints
columns/indexes/foreign keys; `rok schema dump` approximates the live
schema as DDL.

Derive typed models in your code (the `rok-data` facade re-exports the
macros):

```rust,ignore
use rok_data::prelude::*;

#[derive(Debug, Model)]
struct Note {
    id: Uuid,
    text: String,
    created_at: DateTime<Utc>,
}
```

Tests that touch the database use `#[rok_data::test]` (attribute; it
creates a schema-scoped `Db` for the test, like the original
`#[rok_db::test]`).

## 2. Whole-crate generation (`rok generate`)

Instead of hand-writing models, describe tables as `.sql` under `sql/`
and let the generator emit a crate with models, typed queries and the
migration journal wired in:

```console
$ mkdir sql && /* write CREATE TABLE files into sql/ */
$ rok generate --input sql --output db-gen --crate-name notes-db
```

Defaults come from `rok-data.toml` when it exists:

```toml
[gen]
input = "sql"          # .sql sources
output = "db-gen"      # generated crate directory
crate_name = "notes-db"
rok_data_version = "0.1"   # or rok_data_path = "../../crates/rok-data"
```

Flags beat the file; `--strict` fails on generator warnings (queries
without tenant/soft-delete filters); `--migration <name>` appends an
auto-migration for schema changes since the last run (destructive steps
require `--allow-destructive`). CI asserts freshness with:

```console
$ rok generate check --strict
```

## 3. Resource modules (`rok sqlc generate`)

The rok-data-sqlc engine generates modules from a schema file — without
a database connection — into an existing crate:

```console
$ rok sqlc init .        # scaffolds actix-web layout (optional)
$ rok sqlc g             # reads rok-data-sqlc.toml; or --schema schema.sql
```

Each table gets `src/db/<table>/{models,read,write,filter,relationships,…}`,
plus optional API features and OpenAPI schemas (the engine's road map is
its own `docs/guide/`). Its ADR-0016 architecture check:

```console
$ rok sqlc check-arch
```

## 4. Where to read further

- Command contract: `docs/cli-reference.md`
- Coming from rok-db/rok-sqlc/rok-fluent/rok-orm:
  `docs/migration-guide.md`
- Why the workspace looks like this: `docs/architecture.md`,
  `docs/decisions/*.md`
- Engine-side guides (API features, OpenAPI, conventions):
  `crates/rok-data-sqlc/docs/`
