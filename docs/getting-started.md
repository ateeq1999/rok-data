# Getting started with rok-data

This walks through the front door (`rok init` and `rok gen`) and the two
older paths behind it: a rok-db-style application (runtime + whole-crate
generation) and a resource-module application (engine generation).
Prerequisites: Rust ≥ 1.94, PostgreSQL 12+.

## 1. The front door (`rok init` and `rok gen`)

One command scaffolds a project, one generates it. The project's schema
directory is its only source of truth (workspace ADR 0010): the plain `*.sql` files
in it are the schema as it is now, and the staged migration pairs beside
them are the history — how the schema got there.

Start from a schema file:

```console
$ rok init my-api --schema schema.sql
$ cd my-api
```

`rok init` writes the application skeleton (`src/main.rs`, `src/lib.rs`,
`src/app.rs`, `src/config.rs`, `src/error.rs`, `src/openapi.rs`,
`src/shared/`), the settings file, `AGENTS.md`, and the schema directory
`migrations/`, seeded with the schema file you named. Every file it
writes is written once and never rewritten.

The settings file is the unified `rok.toml` (workspace ADR 0011), whose `[generate]`
section holds the generation settings; the engine's own
`rok-data-sqlc.toml` stays readable, and `rok init` writes that one for
now. `rok gen` reads either.

Then generate:

```console
$ rok gen
```

`rok gen` reads the schema directory and writes the data layer
(`src/db/**`), the shared tools and the API features (`src/features/**`),
printing every file it wrote.

Change the schema, and stage the change as a migration:

```console
$ rok gen --migration add_posts
```

That writes `migrations/0002_add_posts.up.sql` and its `.down.sql`: the
difference between the schema and the last staged migration. The generator
reads the truth only, so a staged migration is never applied twice.

In CI, the drift check:

```console
$ rok gen check
```

It writes nothing, reports the missing, changed and stale files, and exits
1 when the generated code is out of date.

A whole API in one step:

```console
$ rok init my-api --template axum
$ cd my-api && rok gen
```

The `axum` template is a complete axum application — the app, its `users`
and `audit-log` features, the OpenAPI document and the tests — and the
`actix` template is its actix-web twin.

The whole chain in one crate: [`examples/api`](../examples/api) holds a
schema of two files, the generated data layer and API features beside it,
and the staged migration pairs. The repository's own gates compile it.

## 2. The runtime, by hand

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

The verbs hang off the model, not a module namespace. Reads take the
`Model` chain, writes take `ModelWrite`, a relation accessor starts its
chain, and a named scope is the entry:

```rust,ignore
// reads: one row by key, the rest as a chain
let note = Note::find(&db, note_id).await?;
let recent = Note::filter(Note::CREATED_AT.gt(a_week_ago))
    .order_by(Note::CREATED_AT.desc())
    .all(&db)
    .await?;

// writes: the row-level verbs of `ModelWrite` (generated models also
// carry the module verbs: `create`, `patch`, `destroy`)
let stored = draft.insert(&db).await?;
stored.delete(&db).await?;

// relations: the generated accessors (one batched query per include)
let author = Post::find(&db, post_id).await?;
let by_the_author = author.posts().all(&db).await?;

// named scopes: `#[rok(scopes(popular = path::to::popular))]` on the model
let popular = Note::popular().all(&db).await?;
```

Tests that touch the database use `#[rok_data::test]` (attribute; it
creates a schema-scoped `Db` for the test, like the original
`#[rok_db::test]`).

## 3. The legacy whole-crate generator (`rok generate`)

This is the superseded path: `rok gen` is the front door's generator, and
`rok generate` keeps its flags, its output and its exit codes (the command
no longer takes the `generate` subcommand, so the write mode is flat).
It is kept for the projects that still use it; its removal is deferred to
1.0 (engine ADR 0012).

Instead of hand-writing models, describe tables as `.sql` under `sql/`
and let the generator emit a crate with models, typed queries and the
migration journal wired in:

```console
$ mkdir sql
$ vi sql/decisions.sql    # your CREATE TABLE files, one or more
$ rok generate --input sql --output db-gen --crate-name my-app-db
```

Defaults come from `rok.toml` when it exists (the legacy `rok-data.toml`
name still works, with a note):

```toml
[generate]
input = "sql"          # .sql sources
output = "db-gen"      # generated crate directory
crate_name = "notes-db"
rok_data_version = "0.2"   # or rok_data_path = "../../crates/rok-data"
```

Flags beat the file; `--strict` fails on generator warnings (queries
without tenant/soft-delete filters); `--migration <name>` appends an
auto-migration for schema changes since the last run (destructive steps
require `--allow-destructive`). CI asserts freshness with:

```console
$ rok generate check --strict
```

## 4. The legacy resource-module generator (`rok sqlc generate`)

The engine's own generator, behind the legacy `rok sqlc …` subtree: the
tokens are forwarded verbatim, the commands keep working exactly as they
did, and their removal is deferred to 1.0 (engine ADR 0012).

The rok-data-sqlc engine generates modules from a schema file — without
a database connection — into an existing crate:

```console
$ rok sqlc init .        # scaffolds actix-web layout (optional)
$ rok sqlc g             # reads rok-data-sqlc.toml; or --schema schema.sql
```

Each table gets `src/db/<table>/{models,read,write,filter,relationships,…}`,
plus optional API features and OpenAPI schemas (the engine's road map is
its own `docs/guide/`). Its engine ADR 0016 architecture check:

```console
$ rok sqlc check-arch
```

## 5. Where to read further

- Command contract: `docs/cli-reference.md`
- Coming from rok-db/rok-sqlc/rok-fluent/rok-orm:
  `docs/migration-guide.md`
- Why the workspace looks like this: `docs/architecture.md`,
  `docs/decisions/*.md`
- Engine-side guides (API features, OpenAPI, conventions):
  `crates/rok-data-sqlc/docs/`
