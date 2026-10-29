---
name: rok-data-init
description: >
  Scaffold and generate a rok-data service end to end. Use when: the user
  wants a new rok-data project, an axum or actix API from a schema, the
  data layer generated from the schema directory (`rok init`, `rok gen`,
  `rok gen check`), a migration staged for a schema change
  (`rok gen --migration`), CI freshness of generated code, rok.toml
  configuration for generation, or a greenfield example running against
  PostgreSQL. Fires for "scaffold", "bootstrap", "generate the data
  layer", "make me an API from this schema" style tasks in rok-data
  projects.
---

## The one flow

The schema directory is the project's only source of truth
(workspace ADR 0010): plain `*.sql` files are the schema as it is now;
the `NNNN_name` pairs beside them are the history.

```sh
# 1. Scaffold (schema seeded into the truth dir).
rok init my-api --schema schema.sql && cd my-api
# or a complete template application instead of the skeleton:
rok init my-api --template axum     # actix is the twin template

# 2. Generate the data layer + API features; prints every file.
rok gen

# 3. Stage a schema delta as a migration pair + regenerate.
#    (destructive steps refuse without --allow-destructive)
rok gen --migration add_posts

# 4. The freshness gate — CI mode, exit 1 on drift, writes nothing.
rok gen check

# 5. Apply to the database.
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/my_api
rok migrate up                      # status: rok migrate status --json
```

**Completion criterion:** `rok gen check` exits 0 and
`rok migrate status --json` shows every pair's `state: "applied"`,
`drift: false`.

## Files `rok init` writes (never rewritten)

`src/main.rs`, `src/lib.rs`, `src/app.rs`, `src/config.rs`,
`src/error.rs`, `src/openapi.rs`, `src/shared/`, the settings file, an
`AGENTS.md`, and the schema directory. Everything is a **scaffolded
file**: written once; a second run fills only what is missing and keeps
edits. `--schema PATH` lands as `schema/0001_init.sql` (an existing one
is kept, never overwritten).

## Settings: `rok.toml` under `[generate]`

| Key | Meaning |
| --- | ------- |
| `schema` | the truth dir (default `schema/`; `migrations/` also fed) |
| `migrations` | the history dir the pairs stage into |
| `output` | where modules are written (default `src/db`) |
| `api.framework` | `actix_web` (default) or `axum` for the generated features |
| `extractors` | must contain the framework's extractor |
| `rok_data_version` xor `rok_data_path` | pin the facade one way only |

The engine's legacy `rok-data-sqlc.toml` is still read; **flags beat the
file everywhere**. Canonical key table: workspace ADR 0011.

## Commands in scope

`rok init`, `rok gen [check] [OPTIONS]` — never `rok generate …` or
`rok sqlc …` for this flow: those are the legacy paths
([`docs/cli-reference.md`](../../../docs/cli-reference.md) has their exact
flags; removal deferred to 1.0, engine ADR 0012).

## Traps

- Edit the **schema**, not `src/db/**` / `src/features/**` (they carry
  `@generated` markers; `--force` is the only overwrite for non-mine
  files: treat it as user-approved only).
- A staged pair is written `0002_…` by the diff engine against the *last
  staged* pair — applying via `rok migrate` never feeds generation back,
  so `rok gen` after the fact is always safe and idempotent.
- One framework per project: switching `[api] framework = "axum"` after
  generating actix features means regenerating (and merging, not keeping,
  the old features).
- Template projects regenerate differently: template files belong to the
  template; hand-written code goes in features/shared, not the db layer.
