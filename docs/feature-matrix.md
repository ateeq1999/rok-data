# Feature matrix

What exists where, after the consolidation. "—" means the capability has
no implementation in the workspace; "mapped" means a documented
equivalent without a ported implementation.

## Runtime capabilities

| Capability                        | rok-db (origin) | rok-fluent (origin) | rok-orm (origin) | rok-sqlc (origin) | rok-data now                        |
| --------------------------------- | --------------- | ------------------- | ---------------- | ----------------- | ----------------------------------- |
| Typed models (derive)             | ✔               | ✔ (own design)      | ✔ (own design)   | —                 | ✔ `rok-data-macros`                 |
| `Expr`/`Select` builder           | ✔               | ✔ (fluent)          | ✔ (fluent)       | —                 | ✔ `rok-data-core`                   |
| Joins/relationships (runtime)     | ✔               | ✔ (fluent)          | ✔ (fluent)       | —                 | ✔ `rok-data-core`                   |
| Migrations: file pairs + journal  | ✔               | —                   | —                | —                 | ✔ core `migration` + `migration_file` |
| Migrations: single-file sections  | —               | ✔                   | —                | —                 | ✔ same loader accepts both          |
| Status rows (state/drift/checksum)| ✔               | —                   | —                | —                 | ✔ (`StatusRow` JSON contract)        |
| Schema introspection              | —               | ✔ (SchemaInspector) | —                | —                 | ✔ core `introspect`                 |
| Metrics                           | ✔               | —                   | —                | —                 | ✔ (metrics gated feature)           |
| `#[rok::test]` DB test attribute  | ✔               | ✔                   | ✔                | —                 | ✔                                   |
| DATABASE_URL only, Postgres 12+   | ✔               | ✔                   | ✔                | —                 | ✔                                   |

## Generation capabilities

| Capability                                    | rok-db-gen (origin) | rok-sqlc (origin) | rok-data now                          |
| --------------------------------------------- | ------------------- | ----------------- | ------------------------------------- |
| Generate model crate from `.sql` + migrations  | ✔                   | —                 | ✔ legacy `rok generate` (`rok-data-codegen`) |
| Flags-beat-file precedence, `--strict` check   | ✔                   | —                 | ✔                                     |
| Per-table resource modules                     | —                   | ✔                 | ✔ legacy `rok sqlc generate`          |
| Front door: data layer from the schema dir     | —                   | ✔                 | ✔ `rok gen` (engine ADR 0012)                 |
| Front door: staged migration pairs             | ✔                   | —                 | ✔ `rok gen --migration`               |
| Front door: CI drift check                    | ✔                   | ✔                 | ✔ `rok gen check` (exit 1 on drift)   |
| Generate from schema file, no DB needed        | —                   | ✔                 | ✔ (parser, no connection)             |
| Queries annotations (`@sqlc`, conflicts)       | —                   | ✔                 | ✔                                     |
| API feature layer (actix, extractors)          | —                   | ✔                 | ✔ (engine feature)                    |
| API feature layer for axum (`[api] framework`)   | —                   | —                 | ✔ (engine feature)                    |
| OpenAPI schemas (`utoipa`)                     | —                   | ✔                 | ✔ (engine feature)                    |
| Init scaffold (dir + templates)                | —                   | ✔                 | ✔ `rok init` (`--template actix`/`axum`) / legacy `rok sqlc init` |
| check-arch (engine ADR 0016 layout rules)             | —                   | ✔                 | ✔ `rok sqlc check-arch`               |
| `@generated` markers, stale detection          | ✔ (`// generated…`) | ✔                 | ✔ both marker generations recognized  |
| Vertical slice: schema → models, features, migrations | —             | —                 | ✔ `examples/api`                      |

The front door is one flow: `rok init` scaffolds a project (the skeleton,
or a whole `--template actix` / `--template axum` application) and seeds
its schema directory; `rok gen` generates the data layer, the shared tools
and the API features from that directory, and stages the migration pairs
with `--migration`; `rok gen check` fails the build when the generated code
is out of date. The API features are generated for one framework per
project: `[api] framework` (or `rok gen -w/--api-framework`) picks
`actix_web` (the default) or `axum`.

## CLI command mapping (old → new)

| Old binary   | Old command        | New command        | Notes                          |
| ------------ | ------------------ | ------------------ | ------------------------------ |
| `rok-db-gen` | `generate`         | `rok generate`     | legacy: flags unchanged        |
| `rok-db-gen` | `check`            | `rok generate check` | exit 1 on drift               |
| `rok-db-gen` | `migrate create`   | `rok migrate create` | writes the file pair         |
| `rok-db-gen` | `migrate status`   | `rok migrate status` | adds `--json`                |
| `rok-db-gen` | `migrate up`       | `rok migrate up`     | adds `--json`                |
| `rok-db-gen` | `migrate down`     | `rok migrate down`   | adds `--json`                |
| `rok-db-gen` | (none)             | `rok migrate rollback` | alias of `down`            |
| `rok-fluent` | `migrate *`        | `rok migrate *`      | single-file sources accepted  |
| `rok-fluent` | `schema dump/diff` | `rok schema dump|diff` | introspect-backed           |
| (new)        | —                  | `rok db check|inspect` | connectivity + structure    |
| (new)        | —                  | `rok init`        | root scaffolding command (engine ADR 0012)   |
| (new)        | —                  | `rok gen`         | root generation command (engine ADR 0012)    |
| (new)        | —                  | `rok gen check`   | CI mode: exit 1 on drift             |
| `rok-sqlc`   | `g`/`generate`     | `rok sqlc g`       | legacy: engine clap grammar verbatim |
| `rok-sqlc`   | `i`/`init`         | `rok sqlc i`       | legacy: as above               |
| `rok-sqlc`   | `a`/`check-arch`   | `rok sqlc a`       | legacy: as above               |

`rok generate` and `rok sqlc …` are the legacy commands: they keep working
exactly as they do now (the `sqlc` tokens are forwarded verbatim to the
engine), they are superseded by the front door (`rok gen` generates,
`rok init` scaffolds), and their removal is deferred to 1.0 (engine ADR 0012).

Exit-code contract: `--help`/`--version` → 0; wrong usage at the root →
2 (clap); usage errors inside `rok sqlc …` → 1 (preserved engine
behavior); runtime command failures → 1.

## Workspace features

| Crate           | default | none | all |
| --------------- | ------- | ---- | --- |
| `rok-data`      | ✔ default set | `--no-default-features` | `--all-features` (chrono, uuid, decimal, json, migrate, serde, axum, metrics, ipnetwork, mac_address) |
| `rok-data-core` | ✔ default set | ✔ | ✔ |
| `rok-data-codegen`, `rok-data-sqlc`, `rok-data-cli`, `rok-data-macros` | plain | ✔ | ✔ |
