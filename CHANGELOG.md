# Changelog

All notable changes to the `rok-data` workspace are documented here. The
format is [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and
the version numbers follow [Semantic
Versioning](https://semver.org/spec/v2.0.0.html). Release notes for a
tag are this file's `## [X.Y.Z]` section (the release workflow checks
that it exists and won't ship a tag without it).

## [0.2.0] — the unified ORM chains through the generated data layer

The front door grows two root commands: `rok init [NAME] [--template
<NAME>] [--schema <PATH>]` scaffolds a project — the application skeleton,
or a ready-made template — and seeds the project's schema directory with
the schema file it is given. Every file it writes is a scaffolded file:
written once, never rewritten.

`rok gen [OPTIONS] [check]` is the unified front door's generator: it
turns the project's schema directory into the data layer (`src/db/**`),
the shared tools and the API features (`src/features/**`), and — with
`--migration NAME` — stages the difference between the schema and the
last staged migration as a `NNNN_name.up.sql` / `.down.sql` pair in the
history directory. `rok gen check` is the CI mode: it writes nothing,
reports the missing, changed and stale files, and fails on drift. The
settings file is the unified `rok.toml` (ADR 0011), whose `[generate]`
section holds the generation settings; the engine's
`rok-data-sqlc.toml` stays readable. The legacy `rok generate` and
`rok sqlc …` keep their flags, their output and their exit codes — the
`sqlc` tokens are still forwarded verbatim to the engine — and the root
help now marks them as superseded by the front door; their removal is
deferred to 1.0. One spelling changed: `rok generate` no longer takes the
`generate` subcommand, so the write mode is flat (`rok generate
[OPTIONS]`) and the CI mode nests (`rok generate check`).

The truth and the history now live in one directory each: `rok gen`
reads the truth directory — the plain `*.sql` files a hand writes,
`schema/` in the layout `rok init` seeds — and, with `--migration NAME`,
stages the full history into the history directory (`migrations` by
default): the `NNNN_name.up.sql` / `.down.sql` pairs and, hidden beside
them, the `.rok-schema.json` snapshot of the staged shape that the next
staging diffs against. `rok migrate` reads that history unchanged. Both
directories keep their pre-split default — one `migrations` directory
holds the truth and the pairs in an old project, and it keeps
generating and staging as before — and the split is wired in the
settings: the truth is named by `--schema` / `[generate] schema` as
before, the history by the new `[generate] migrations` key and `rok gen
--migrations-dir` (the flag beats the file). The loader that reads the
history skips hidden files, so the snapshot rides along unseen — and the
snapshot itself leaves the truth's `-- rok:down` undo statements out.
Where `--migration`
used to advise a `psql` invocation, it now points at the pair it staged:
`apply the staged pair (<up>) with `rok migrate up``, with `--dir`
added only when the history is not the default. `rok init` seeds the
truth (`schema/0001_init.sql`, whose `-- rok:down` lines mark its undo
SQL) and no history — the history appears with the first staged pair —
and the templates and [`examples/api`](examples/api) follow the same
layout: a `schema/` truth beside the pairs and the snapshot in
`migrations/`.

The generated per-table data layer (`models`, `read`, `write` and the
relationship loaders) now rides the `rok-data` model chains instead of
built SQL: the functions that ride the chains return `rok_data::Error`
where they used to return `sqlx::Error`. Joins, search/listing/filter and
`m2m` pair loaders stay on the Query builder for this slice, and still
return `sqlx::Error`. The actix template's skeleton gains the `rok-data`
dependency and an `AppError` conversion.

The generated API features have a second backend: `[api] framework =
"axum"` (or `-w/--api-framework axum`) generates them for axum instead of
actix-web. The routes are a `Router`'s, the handlers take axum's
extractors, and the shared tools (health, the tenant, the validated
bodies) are axum's. The bodies, the service and the OpenAPI document are
the same either way, and actix-web stays the default: every existing
project generates exactly what it did before. `rok sqlc check-arch`
follows this key as well: its rule 1 polices the imports of the API
backend the project is generated for (`actix_web` under the default,
`axum` under `framework = "axum"`), over the same allowed-file set either
way, so an axum project's architecture is checked like an actix one's
instead of passing rule 1 trivially.

`rok init --template axum` scaffolds the starter that matches it: the
`actix` template's twin, with the same `users` and `audit-log` features,
the same wire standard and the same contract test, written for axum. The
two templates differ only in the framework's edge — the routes, the
handlers, the extractors and the server setup — and both are checked in
CI: regenerated, linted, `check-arch`ed and tested.

The vertical slice of the pipeline is checked in as an example:
[`examples/api`](examples/api) holds a schema of two files, the generated
data layer and axum API features beside it, and the staged migration
pairs — one crate, from `rok init` and `rok gen` to a running API. It is a
crate of its own (workspace-excluded, like the templates and the
fixtures), and the repository's gates compile it: the drift check, the
staged pairs, rustfmt, the lint gate and its tests.

Also in 0.2.0: an unqualified `ALTER TABLE <name>` in a later
migration now reaches a table created under a schema
(`catalog.products`) when that SQL name is unique in the schema, and
diagnostics name the resolved table; a truth file can carry an
`ALTER TABLE` beside its `CREATE TABLE`s — the schema parser now skips
the `-- rok:down` undo statements the truth carries (previously the undo
was applied as a statement of its own and undid the truth's change), and
refuses a misplaced marker with the whole-crate generator's rule
wording; the `Model` derive accepts
no-primary-key tables; composite types (money, ranges, timetz,
inet/cidr, macaddr, bytea arrays) ride the new `ipnetwork` and
`mac_address` facade features.

The release is a tag: `vX.Y.Z` must equal the workspace's version, and
the tag publishes the `rok` binary for Linux (musl), macOS and Windows
(x86_64 and ARM64) as checksummed archives, then the six crates to
crates.io in dependency order (ADR 0008). The repository's `install.sh`
and `install.ps1` fetch and verify those archives, so a release installs
with one line.

## [0.1.0] — initial consolidation

First release of the consolidated workspace. `rok-data` combines the
former `rok-db`, `rok-sqlc`, `rok-fluent` and `rok-orm` repositories
(their own histories and releases stay in their repositories; see
`docs/migration-guide.md` for every rename and its exact consequences).

### Added

- The `rok` CLI (`crates/rok-cli`), the ecosystem's single user-facing
  binary: `generate`, `generate check`, `migrate
  create|status|up|down`, `db check|inspect`, `schema dump|diff`, and
  the `rok-data-sqlc` engine behind `rok sqlc
  generate|init|check-arch`.
- `rok-data-core::{introspect, migration_file}`: catalog introspection
  (tables, columns, indexes, foreign keys, DDL dump) and the unified
  migration file source (file pairs plus single-file sections).
- A doc gate in CI: `cargo doc` over every crate with the same rustdoc
  strictness docs.rs builds with.

### Renamed (breaking)

- Crates, environment prefix `ROK_*`, journal, and config files keep
  their semantics but carry the new names: `rok-data*` crates,
  `rok-data.toml` and `rok-data-sqlc.toml`, journal
  `_rok_data_migrations`.
- Generated-file headers: `@generated by rok-sqlc` →
  `@generated by rok-data-sqlc` (both historical markers stay valid).
- MSRV unified to 1.94 (rok-sqlc previously supported 1.88; the
  workspace floor is the one line the generated code needs anyway).

### Removed

- The standalone binaries `rok-db-gen` and `rok-sqlc` (merged into
  `rok`, ADR 0002).
- `rok-fluent` and `rok-orm` as separate runtimes (superseded; the
  absorbed pieces are documented in ADR 0001).
