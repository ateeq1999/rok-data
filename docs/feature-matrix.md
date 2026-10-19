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
| Generate model crate from `.sql` + migrations  | ✔                   | —                 | ✔ `rok generate` (`rok-data-codegen`)  |
| Flags-beat-file precedence, `--strict` check   | ✔                   | —                 | ✔                                     |
| Per-table resource modules                     | —                   | ✔                 | ✔ `rok sqlc generate`                 |
| Generate from schema file, no DB needed        | —                   | ✔                 | ✔ (parser, no connection)             |
| Queries annotations (`@sqlc`, conflicts)       | —                   | ✔                 | ✔                                     |
| API feature layer (actix, extractors)          | —                   | ✔                 | ✔ (engine feature)                    |
| OpenAPI schemas (`utoipa`)                     | —                   | ✔                 | ✔ (engine feature)                    |
| Init scaffold (dir + templates)                | —                   | ✔                 | ✔ `rok sqlc init`                     |
| check-arch (ADR 0016 layout rules)             | —                   | ✔                 | ✔ `rok sqlc check-arch`               |
| `@generated` markers, stale detection          | ✔ (`// generated…`) | ✔                 | ✔ both marker generations recognized  |

## CLI command mapping (old → new)

| Old binary   | Old command        | New command        | Notes                          |
| ------------ | ------------------ | ------------------ | ------------------------------ |
| `rok-db-gen` | `generate`         | `rok generate`     | flags unchanged                |
| `rok-db-gen` | `check`            | `rok generate check` | exit 1 on drift               |
| `rok-db-gen` | `migrate create`   | `rok migrate create` | writes the file pair         |
| `rok-db-gen` | `migrate status`   | `rok migrate status` | adds `--json`                |
| `rok-db-gen` | `migrate up`       | `rok migrate up`     | adds `--json`                |
| `rok-db-gen` | `migrate down`     | `rok migrate down`   | adds `--json`                |
| `rok-db-gen` | (none)             | `rok migrate rollback` | alias of `down`            |
| `rok-fluent` | `migrate *`        | `rok migrate *`      | single-file sources accepted  |
| `rok-fluent` | `schema dump/diff` | `rok schema dump|diff` | introspect-backed           |
| (new)        | —                  | `rok db check|inspect` | connectivity + structure    |
| `rok-sqlc`   | `g`/`generate`     | `rok sqlc g`       | engine clap grammar verbatim   |
| `rok-sqlc`   | `i`/`init`         | `rok sqlc i`       | as above                       |
| `rok-sqlc`   | `a`/`check-arch`   | `rok sqlc a`       | as above                       |

Exit-code contract: `--help`/`--version` → 0; wrong usage at the root →
2 (clap); usage errors inside `rok sqlc …` → 1 (preserved engine
behavior); runtime command failures → 1.

## Workspace features

| Crate           | default | none | all |
| --------------- | ------- | ---- | --- |
| `rok-data`      | ✔ default set | `--no-default-features` | `--all-features` (metrics) |
| `rok-data-core` | ✔ default set | ✔ | ✔ |
| `rok-data-codegen`, `rok-data-sqlc`, `rok-cli`, `rok-data-macros` | plain | ✔ | ✔ |
