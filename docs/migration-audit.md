# Migration audit

What the four source repositories were at the time of the consolidation,
verified by reading implementations and running their tests rather than
trusting names. Every finding below drove a decision in
[architecture.md](architecture.md), [migration-plan.md](migration-plan.md)
or an [ADR](decisions/).

Sources (each kept as `*_src` sibling clones for reference):

| Repository                      | Role in the original ecosystem                                        |
| ------------------------------- | --------------------------------------------------------------------- |
| `github.com/ateeq1999/rok-db`   | sqlx 0.9 runtime: typed models, migrations, DB bootstrap              |
| `github.com/ateeq1999/rok-sqlc` | schema-parsing SQLx module generator with its own CLI and test corpus |
| `github.com/ateeq1999/rok-fluent` | early ORM-style prototype (fluent query builders, own migrations)   |
| `github.com/ateeq1999/rok-orm`  | second prototype, superseded by both of the above                      |

## rok-db — the runtime (most mature)

- sqlx **0.9**, PostgreSQL only, workspace with `rok-db-core` (runtime:

  `Db`, `Expr`/`Select` builder, migrations, metrics), `rok-db-macros`
  (derive + attribute macro `#[rok_db::test]`), a facade crate `rok-db`,
  and `rok-db-codegen` (the `rok-db-gen` binary: generates a model crate
  from `.sql` files plus a migrations journal integration).

- Tests green at the port: 9 crates, doc tests 34 ignored (DB-gated), the

  remaining suite needs `DATABASE_URL` and prints skip notes without it.

- Conventions worth preserving: journal table `_rok_db_migrations` with

  state/checksum/drift tracking; `NNNN_name.up.sql` / `.down.sql` pairs;
  flags-beat-file configuration precedence in the generator; `strip =
  true` in the release profile.

## rok-sqlc — the module generator (feature-richest)

- Parses `CREATE TABLE`/`CREATE TYPE`/`ALTER TABLE` SQL with a

  hand-written PostgreSQL parser (no database needed to generate), emits
  per-table resource modules (`models|read|write|filter|relationships`
  …), API features (actix handlers), OpenAPI schemas, an actix-web
  application template, `init` scaffolding, and `check-arch`.

- 471 files, ~30k lines; its CLI grammar is test-proposed and stable
  (`rok-sqlc generate|init|check-arch`, aliases `g|i|a`).

- Extensive suites: 236 library tests, a 100+ module corpus compiled as a

  fixture crate, compile fixtures (`fixture-app`, `api-app`),
  milestone-diff records, README examples that are executed.

- Its `@generated` header carries the tool name; staleness checks and

  `init` rely on recognizing it.

## rok-fluent — superseded prototype

- Fluent API design (`query.filter(...).order_by(...)`), its own

  single-file `-- up / -- down` migration convention, `SchemaInspector`
  for catalog introspection, and its own CLI.

- **Decision: superseded by the rok-db lineage.** What survives is

  absorbed wholesale: catalog introspection (ported into
  `rok-data-core::introspect`) and the single-file migration format (the
  unified loader accepts it; see ADR 0004). Nothing else: conduit to its
  own runtime was never completed and rok-orm/rok-db cover the same
  ground.

## rok-orm — superseded prototype

- Second ORM generation; the live code paths were already folded into

  rok-db's design vocabulary (entities, repositories) before this
  consolidation. **Decision: superseded; migration guide records the API
  mapping only.** No port beyond documentation.

## Cross-cutting findings

- Two SQLx module generators coexist (rok-db-gen emits an entire model
  crate from migrations; rok-sqlc emits per-table resource modules from a
  schema file). They serve different jobs; both are kept as separate
  libraries behind one CLI (ADR 0006).
- Two migration conventions coexist (file pairs + journal in rok-db;
  single-file sections in rok-fluent). One engine now loads both
  (ADR 0004).
- Every repository duplicated: naming rules, PostgreSQL types mapping,
  "you don't need a database to generate" philosophy, MIT OR Apache-2.0
  licensing, and MSRV/deps. The workspace keeps the strictest of each:
  sqlx 0.9, rust-version 1.94 (same floor as generated code), lints as
  in the rok-db workspace.
- rok-sqlc's binary-driven tests cannot live in the engine crate anymore
  (`CARGO_BIN_EXE_rok-data-sqlc` does not exist once the binary target is
  removed); they moved to `rok-cli/tests` (ADR 0003).
