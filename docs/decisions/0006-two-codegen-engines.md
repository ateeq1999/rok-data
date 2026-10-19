# ADR 0006: two generator engines, one CLI, two jobs

Date: 2026-10-19 · Status: accepted

## Context

The consolidation inherits two "code generators":

- `rok-db-codegen` — reads `.sql` model sources plus the migrations

  journal and emits an **entire model crate** (models, typed queries,
  migrations, a generated `Db` bootstrap). Its CLI was `rok-db-gen`.

- `rok-data-sqlc` — parses a PostgreSQL schema file (no database) and
  emits **per-table resource modules** under `src/db/<table>/…`, with
  optional API features, OpenAPI, and an architecture checker.

Merging them into one engine would be the maximal change with the least
benefit: their inputs, outputs and compile-time stories differ, and each
was developed and tested to a high standard separately.

## Decision

Keep both as reusable libraries with distinct jobs and non-competing
config:

| facet         | `rok generate` (codegen)            | `rok sqlc generate` (engine)      |
| ------------- | ------------------------------------- | --------------------------------- |
| input         | `.sql` files + migrations             | one schema file (or migrations)   |
| output        | a whole new crate (models + queries)  | `src/db/…` modules in your crate  |
| config file   | `rok-data.toml` (`[gen]`)             | `rok-data-sqlc.toml`              |
| needs DB      | describe pass (connection)            | no (parser-only)                  |
| flags vs file | flags beat file; `--strict`           | flags beat file                   |

`rok generate` keeps rok-db-gen's exact option semantics (strict ORs
with the file, `rok_data_version`/`rok_data_path` mutually exclusive,
`--rename table.old=new` style renames).

## Consequences

- Two config file names exist by design; generators never read each

  other's config (verifiable: each engine's `config_file` module).

- Users typically pick a lane at project start: whole-crate generation

  (`rok generate`) for rok-db-style applications, in-crate modules

  (`rok sqlc`) for actix-resource-style applications. Nothing prevents

  using codegen's output crate and sqlc's modules in one project.
- Both outputs share the compiler guarantees: generated code is the
  consumer's code, checked by clippy in CI (see ci.yml).
