# Changelog

All notable changes to the `rok-data` workspace are documented here. The
format is [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and
the version numbers follow [Semantic
Versioning](https://semver.org/spec/v2.0.0.html). Release notes for a
tag are this file's `## [X.Y.Z]` section (the release workflow checks
that it exists and won't ship a tag without it).

## [Unreleased]

**Changed**

- A schema statement rok can't undo by itself, left without its
  `-- rok:down`, is refused by its first line, and the message says to
  add the block immediately after that statement ("rok can't undo
  `CREATE TRIGGER books_audit …` by itself; add `-- rok:down` and the
  SQL that undoes it immediately after this statement"). The guide's
  *After a schema change* shows two triggers, each with its own block.
- The Rust Client types an integer by its contract `format`: `int32` is
  `i32` (a `smallint` too), `int64` or no format stays `i64`.
- The TypeScript Client spells an `Option<T>` (a `oneOf` of `null` and
  one schema) as `T | null` and `TSchema.nullable()`, a nullable success
  included: `GET /me/tables/{id}/bill` declared `Option<BillResponse>`
  returns `Promise<BillResponse | null>`. "None yet" is `200` with
  `null`; a 404 still means no such resource.
- A Client's body field that may be left out and may be `null` is
  three-state only in a `PATCH` body; elsewhere it is plain optional, so
  a `POST` reads `Some(value)` in Rust and `name` in Dart instead of
  `Some(Some(value))` and `Nullable(name)`.
- The TypeScript Client's types are zod 4 schemas (`ThingSchema`, with
  `type Thing` inferred from it, names unchanged), carrying the server's
  rules (lengths, bounds, patterns, formats) for forms to check input
  with, and every call checks its JSON success, rejecting with
  `ContractError` when the body is not what the contract describes
  (unknown keys are dropped). A web App depends on `zod@^4`.

**Added**

- Every Client pages a list for you: TypeScript
  `useInfiniteQuery(pagers.notes.list({ q }))`, Dart
  `api.notes.listPager(q: q)` (a `Pager` with `load()`, `more()` and a
  `stream` of the rows), Rust `client.notes().list_pages(&query)` (`while
  let Some(page) = pages.next().await`). Both list shapes are read:
  page numbers and `[api] cursor` cursors.
- A contract operation whose answers are redirects (no `2xx`, a `3xx`),
  such as a sign-in's start and callback at an OpenID provider, is a
  Browser route: `rok gen` accepts it instead of failing with "no success
  response", and each Client builds its URL instead of calling it
  (`api.auth.oidcStartUrl(provider, { return_to })` in TypeScript,
  `api.auth.oidcStartUrl(provider, returnTo: ...)` in Dart,
  `client.auth().oidc_start_url("google", &query)` in Rust).
- Queries: `sum`, `avg`, `min` and `max` are typed by PostgreSQL's rules
  in a `SELECT`, a CTE and a subquery alike (`sum(stock)` over an
  `integer` is `Option<i64>`, `coalesce(sum(stock), 0)` is `i64`), so
  they no longer need a cast. A cast still decides the type.
- `rok sqlc query add NAME @q.sql` reads the SQL from a file, and
  `query add NAME -` from stdin, so SQL with quotes needs no shell
  quoting.
- `rok_data::transaction(&pool, |tx| …)` runs a closure in a transaction
  on a plain `PgPool` (what a service holds), committing on `Ok` and
  rolling back on `Err`; its `tx` is a `rok_data::Tx`. `check-arch` lets
  a service control its transactions (`sqlx::Transaction`, a savepoint's
  `begin`) and still fails `sqlx::query` there.
- `docs/validation.md`, a guide to validating input with
  `#[derive(Input)]`: field rules and transforms, database rules,
  context, conditional rules, nesting, messages, the 422 body, JSON
  Schema, and what generated APIs do with it. Its samples are compiled.
- `[clients]` entries may be tables: `ts = { dir =
  "apps/web/src/lib/api", layout = "sources" }` emits the TypeScript or
  Dart Client's source files flat into an App's own directory, without
  the package around them; `rok gen` owns only its marked files there
  (`docs/cli-reference.md`).

- Typed queries support data-modifying CTEs (`WITH x AS (UPDATE …
  RETURNING …) INSERT … SELECT … FROM x`): each CTE's `RETURNING` columns
  are known to the statements after it, and the final statement types the
  result (`:execrows`, or its `RETURNING`).
- Typed queries support `UNION`, `UNION ALL`, `INTERSECT` and `EXCEPT` at
  the top level, in subqueries and as an `INSERT … SELECT` source, each
  column of the type its branches share; mismatched branches fail naming
  the column.
- Queries type `generate_series`, `unnest` and `jsonb_to_recordset` in
  `FROM`, so one statement writes many rows: a copy-forward `INSERT …
  SELECT … FROM generate_series($1::date, $2::date, '7 days') AS week`,
  or a batch from slices (`unnest($2::uuid[], $3::int[]) AS l(item_id,
  quantity)`) or from one JSON parameter (`jsonb_to_recordset($2) AS
  l(item_id uuid, quantity int)`, a `serde_json::Value`). The engine
  README's "Writing query files" documents the patterns.
- One query serves "upcoming or past": a `bool` switch
  (`CASE WHEN $2::bool THEN … ELSE … END` in `WHERE`, one `CASE` per
  sort direction in `ORDER BY`) types its placeholder, also when it
  appears only in `ORDER BY`. The engine README documents the pattern.

**Fixed**

- A partial update's `coalesce($2, locale)` types `$2` as the column's
  `Option<T>` (a CHECK-list enum included) instead of asking for
  `-- param:`.
- A `rok gen` whose contract export is a `cargo` command, and fails
  because the App no longer compiles (a data-layer change broke a call
  site), says so: "the App doesn't build after this change: fix the call
  sites, then run `rok gen` to write the contract and the Clients",
  followed by the compiler's first error. `--json` carries it as the
  failed export step's `error`. Other export failures keep their message.
- `rok gen` and `rok gen check` fail when two `ToSchema` types in the
  App share a component name (a hand-written DTO named like a generated
  response, say), naming both files, instead of the OpenAPI document
  silently keeping one of them and the Clients coming out wrong. Rename
  one of the types or give it `#[schema(as = ...)]`.

## [0.9.0] — strict, better-typed `rok gen`; one rule for finding the project; `:page` and shared queries

Breaking for some projects. `rok gen` fails on a table or query it can't
generate (`--lenient` skips as before); casts, literals and `coalesce`
are typed more closely, so some generated fields lose their `Option`; a
foreign key without `_id` names its relationship after the referenced row
(`reviewed_by_user`); `rok migrate` and `rok schema diff` find the
project from `rok.toml`; a versioned table's `PATCH` takes `If-Match` in
every Client; and the engine's library API grows fields and arguments.
Follow `docs/migration-guide.md` §To 0.9.0: regenerate, then fix what
the compiler reports.

**Added**

- `rok init my-api --template grpc` (and `rok sqlc init --template
  grpc`) writes the `grpc-api` project from rok-templates 0.7.0: a tonic
  server over the axum template's schema and data layer, with a caller
  check, health, reflection and a streaming RPC. `rok gen` writes each
  Resource's `.proto` and tonic service; its `build.rs` compiles the
  protos with protox, so no `protoc` is needed. An unknown template's
  error now lists `grpc`.
- `:page` queries: `-- name: PartnerBookings :page` over a `SELECT`
  with its `WHERE` and `ORDER BY` generates
  `partner_bookings(&pool, params, page, per_page)`, returning
  `rok_data::Page<PartnerBookingsRow>` with the page's rows and the
  total across every page (an empty page past the end keeps it), the
  same `Page` the generated API serves. Its own `LIMIT`/`OFFSET` is an
  error naming it.
- `Raw::fetch_page(conn, page, per_page)` in the runtime: a raw
  statement a page at a time, with the total counted over the same
  statement and parameters (`.total_from(sql)` counts with your own).
  It runs both on one connection of `&db`, `&pool` or `&mut tx`: `&Db`
  and `&mut Tx` now implement `sqlx::Acquire`.
- Shared reads: `queries/shared/<name>.sql` generates
  `db::queries::shared::<name>`, which any feature may call
  (`check-arch` allows it, and still refuses another feature's query
  module). It holds reads only: a `:exec`/`:execrows` query or an
  INSERT, UPDATE, DELETE or MERGE fails `rok gen`, naming the file, the
  line and the query.
- `rok gen` announces a table the API serves for the first time, once,
  on stderr: ``warning: now serving `/api/v1/posts` (list, create,
  get_one, update, remove); add `posts` to `[api] exclude` to keep it
  private``, and `--json` carries it in `newly_served`. Every table is
  served by default, so a new migration's table no longer goes public
  unnoticed. `[api] include` is the allowlist alternative to `exclude`
  (setting both is an error naming both).
- When a table's API feature is no longer generated (the table excluded
  or dropped), `rok gen` removes its scaffolded `service.rs` and the
  folder too while the file is still exactly as scaffolded (CRLF aside),
  where it left an orphan no `mod` declared. An edited one stays and is
  listed on stderr (`warning: src/features/users/service.rs is no longer
  wired in: ...`) and in `--json`'s `unwired`; `rok gen check` reports
  the unedited one as stale and lists the edited one without failing.
- Typed queries (rok-data-sqlc) support `INSERT INTO t (cols) SELECT …`,
  where it failed "use VALUES": each selected value is typed from the
  column it fills, as with `VALUES`, with `ON CONFLICT` and `RETURNING`.
  A `SELECT` returning the wrong number of values fails naming both
  counts.
- `-- rok:skip [note]` after a query's `-- name:` leaves it ungenerated
  on purpose: `rok gen` prints a `note:` instead of failing without
  `--lenient`, and `--json` lists it under `skipped_on_purpose`, apart
  from `failures` and `skipped`.

**Changed**

- `check-arch` rule 4's message adds where a helper goes: "…declare a
  port in its service.rs instead; a pure helper with no feature state
  belongs in shared/". The rule is unchanged.
- `rok gen` prints `exporting the contract (building the App, slow the
  first time)…` on stderr before the contract export, so a long first
  build isn't silent (`--json`'s stdout is unchanged).
- The Rust Client documents an operation whose sign-in is optional
  (`security((), ("bearer_auth" = []))`, OpenAPI `[{}, {"bearer_auth":
  []}]`) as "Sends the session's token when there is one."; a
  required-auth operation keeps "Sends the session's token.". The TS
  and Dart Clients make no such claim.
- Query parameter names (rok-data-sqlc): a `BETWEEN` bound is
  `<column>_from` / `<column>_to` (was `<column>` / `<column>_2`);
  `-- param: $2 from` renames without restating the type, and
  `-- param: $2 reviewer!` also makes a nullable column's value required
  (`Uuid`, not `Option<Uuid>`).
- Typed queries (rok-data-sqlc) support `UPDATE … FROM` and
  `DELETE … USING`, typed like the equivalent join, and a placeholder in
  arithmetic with a numeric column takes its type. A cast to a typed
  id's own plain type keeps the typed id (`coalesce(p.cover_media_id,
  (SELECT ph.media_id …))::uuid` is `Option<MediaId>`, not
  `Option<uuid::Uuid>`), `coalesce` takes the type its arguments share,
  a scalar subquery its column's, `EXISTS (…)` / `NOT EXISTS (…)` are a
  never-null `bool`, and placeholders inside subqueries are typed.
- Casts keep nullability in generated queries (rok-data-sqlc, a change
  to the generated API). `count(*)::bigint AS owners` in a `:one` query
  is `i64`, where every cast was `Option`; a cast of a `NOT NULL` column
  is not `Option` unless the column is on the outer side of a join; and
  `coalesce(x, <literal>)` is never NULL. Any other cast stays `Option`,
  and a `-- column:` override settles what inference can't (the engine's
  README, "Writing query files").
- `rok gen` and `rok gen check` fail on a table or query they can't
  generate, where they skipped it with a warning that scrolled past. A
  query whose result column can't be typed (`expires_at <= now() AS
  expired`) names its file and line, the expression and the override:
  ``add `-- column: expired <RustType>` ``. `--lenient` skips as before;
  `--strict` is the default now and still accepted. `--json` carries the
  failures in `failures`, and each failure and skipped entry has its
  `path` and `line`. `--strict` no longer fails on the other warnings
  (a scaffolded file using what the settings stopped generating).
- One rule for finding the project: `rok gen`, `rok gen check`,
  `rok migrate *`, `rok schema diff` and `rok sqlc check-arch` walk up to
  the nearest `rok.toml`. Started inside an App a Monorepo's `[apps]`
  names, they act on that App; at the root, on its App(s). So
  `rok migrate up` at a Monorepo root applies `apps/api/migrations`
  instead of failing on a missing `./migrations`, and `check-arch` finds a
  `rok.toml` project (it read only `rok-data-sqlc.toml` before). An
  explicit `--dir` still wins; the new `--app <name>` on `rok migrate *`
  and `rok schema diff` picks an App, and a root naming several Apps asks
  for it. Each command prints one line on stderr naming the settings file
  and directories it read (`using App api: apps/api/rok-data-sqlc.toml
  (migrations apps/api/migrations)`). The rule is `rok_data_sqlc::Project`.
- `rok` loads the App's `.env` for its database commands: `rok migrate
  status|up|down`, `rok schema *` and `rok db *` read `apps/api/.env` (or
  the `.env` beside a plain project's settings file), so `rok migrate up`
  at a Monorepo root finds the App's `DATABASE_URL` without an `export`.
  A variable already in the environment wins, the `using` line ends `,
  loaded apps/api/.env` (`rok db *` and `rok schema dump` print `loaded
  apps/api/.env`), and `--no-env-file` reads nothing. Since `rok db drop`
  guards the database `DATABASE_URL` names, it now guards the `.env`'s too.

**Fixed**

- `rok gen` emits the Clients for an App with a versioned table, whose
  `PATCH` takes `If-Match`: the contract reader refused header
  parameters. A Client takes the header in the call's options:
  `api.documents.update(id, body, { ifMatch: version })` in TypeScript,
  `update(id, body, ifMatch: version)` in Dart,
  `update(&id, &body, &version)` in Rust (a bare version is quoted as an
  entity tag).
- A documented `Option<SomeEnum>` field keeps its comment in the
  TypeScript, Dart and Rust Clients (utoipa puts the description inside
  the `$ref` branch of its `oneOf` with `null`).
- Query-parameter enums reach the contract: generated features register
  every enum their path parameters take, and when `rok gen` finds a `$ref`
  the contract doesn't define (a hand-written `IntoParams` struct's
  `status: Option<RefundStatus>`), the error names the line to add:
  ``register it in `openapi.rs`: `components(schemas(crate::features::refunds::dto::RefundStatus))` ``.
- "Next steps" run as printed: `rok gen` prints `rok migrate up` bare
  when the project rule finds the staged pair's history dir from where it
  ran (a Monorepo root printed `--dir apps/api/migrations`), `--app
  <name>` at a root naming several Apps, and `--dir` only for a history
  dir a flag named; `rok init <dir>` starts its steps with `cd <dir>`.
- A failed `rok gen` step says what's done and what isn't: a run of
  several steps (each App's data layer and migration, the contract export,
  each Client) that fails ends with the steps done, the one that failed
  with its error, the ones skipped, and the `rok gen` command that
  finishes the rest (without `--migration` once it is staged). Nothing is
  rolled back. `--json` carries the same as `{"steps": [...], "finish":
  ...}`. A failed staging prints the data layer's report too, and its
  `--json` body, where it printed nothing.
- An App whose settings leave `output` unset generates into its own
  `src/db`, not the `src/db` of the directory `rok gen` ran in; and a
  Monorepo's `rok gen --json` no longer prints the `api:` heading on
  stdout, so stdout is one JSON document.
- Errors show their causes: the CLI prints an error with its whole chain,
  so `rok gen` on a schema with a syntax error says `error: failed to
  parse SQL schema: schema/0001_init.sql:2: invalid PostgreSQL SQL
  syntax: <the parser's message>` instead of stopping at `failed to parse
  SQL schema`. The same goes for `--json` runs and for the database
  errors of `rok migrate *`, `rok db *` and `rok schema *`.
- `rok sqlc check-arch` says it passed: a clean run prints `checked 3
  features: no violations` on stdout (`, 2 warnings` added when
  `--lenient` let layout violations through), where it printed nothing.
  Its failures and exit codes are unchanged.
- Everyday expressions in generated queries are typed (rok-data-sqlc),
  where `rok gen` failed with the `-- column:` hint: `expires_at <= now()
  AS expired` is `bool` on a `NOT NULL` column and `Option<bool>` on a
  nullable one. Comparisons and boolean logic are `bool`, arithmetic
  takes its operands' type (`numeric` over integers), `CASE` the type its
  branches share, and `||` is text; each is `Option` when an operand can
  be NULL (`IS NULL` never is). A cast literal (`0::int`) is no longer
  `Option`, a change to the generated API. Anything else still fails
  with the hint.
- Calls to the schema's own functions are typed in generated queries
  (rok-data-sqlc): with `CREATE FUNCTION tz_today() RETURNS date` in the
  schema dir, `SELECT tz_today() AS today` generates a function returning
  `Option<NaiveDate>`, and `-- column: today chrono::NaiveDate` makes it
  not-null. A function the schema doesn't define still fails with the
  `-- column:` hint.
- `SCHEMA_SQL` (the generated `db/schema.rs`) is in an order that loads
  into an empty database. It listed the schema files' statements in
  file-name order, so a table referencing one a later file creates
  (`users.home_city_id` in `0001` → `cities` in `0007`) broke the tests
  that run it ("relation cities does not exist"). A statement naming an
  object a later statement creates now moves after it, with the
  statements that need it; each keeps its text. A schema already in a
  loadable order keeps its snapshot as it was; the doc comment no longer
  says it is never executed.
- Line endings don't change migrations. Migration staging (`rok gen
  --migration`) normalises CRLF to LF in the truth files and in the
  `.rok-schema.json` snapshot before comparing, as the engine does with
  its schema and query files: a CRLF checkout whose function and trigger
  equal the snapshot's stages nothing, where it staged a `DROP TRIGGER`,
  a `DROP FUNCTION` and both `CREATE`s again. A snapshot recorded from a
  CRLF file compares equal to the LF text.
- A migration's checksum (`Migration::checksum`, the journal's
  `checksum` column) is computed over its `up` SQL with CRLF normalised
  to LF, so a CRLF and an LF checkout of one `.up.sql` have the same
  checksum; an LF file keeps the checksum it had. A journal row recorded
  from a CRLF file by an earlier version still matches (whether the file
  is checked out with CRLF or LF now), and the next `rok migrate up`
  (`migration::apply`) records the new checksum in its place.
  `docs/migration-guide.md` §To 0.9.0 says what to do for a
  file with mixed line endings.
- A foreign key without `_id` names its relationship after the column
  and the referenced row (rok-data-sqlc): `offers.reviewed_by` referencing
  `users` gives `reviewed_by_user`, where the relationship was named
  `reviewed_by` and `rok gen` failed on its collision with the search
  filter's field for the column. Two such columns to one table
  (`created_by`, `reviewed_by`) give two relationships; columns ending in
  `_id` keep their names. A remaining collision names the
  `[relationships]` entry that fixes it.

## [0.8.1] — gRPC services compile with an immutable column

**Fixed**

- `[api] framework = "grpc"` with an `[api.immutable]` column now
  generates code that compiles: the feature's `Update` RPC no longer
  leaves out the update body's field for the column. The column stays
  unchangeable over gRPC, as over HTTP.

## [0.8.0] — the gRPC framework; `rok server update` and command examples

**Added**

- The engine's third API framework, `[api] framework = "grpc"` (engine
  ADR 0028): a run writes one `proto/<resource>.proto` per Resource (its
  messages and a unary `Get`/`List`/`Create`/`Update`/`Delete` service)
  and, in each feature, a `grpc.rs` that serves it as a tonic service over
  the feature's service trait, in place of the HTTP routes and handlers.
  `shared/grpc.rs` maps an `AppError` to a status; the drift check and
  `check-arch` cover the protos and the services. `ApiFramework::Grpc` is
  the library side, and `ApiFramework::extractor` now returns an `Option`
  (`None` for `Grpc`). The engine's CHANGELOG lists the details;
  `docs/migration-guide.md` §To 0.8.0 says what to change.
- `rok server update <NAME>`: change a registered server's settings
  (`--host`, `--port`, `--user`, `--database`, `--sslmode`, or the whole
  connection with `--url`), its password (`--password-prompt`, or the one in
  `--url`), or both. Settings not given keep their values; no change at all
  is a usage error (exit 2).
- Every command's `--help` ends with an **Examples** section: two or so
  real invocations, each with what it does (`-h` stays short). They live in
  `rok_data_cli::examples` (`EXAMPLES`, `examples(path)`), which tools that
  document the CLI read too; a test parses each example against the
  grammar and requires examples for every command but the legacy `sqlc`
  subtree.

**Fixed**

- A server URL without a database path (`postgres://user@host`) registers
  no default database, as the CLI reference says, instead of an empty one.

## [0.7.0] — `rok-db` and `rok-gen`; binaries and installers move to rok-v2

**Added**

- Two specialized binaries beside `rok-data`, from the same crate:
  `rok-db` (`server`, `db`, `migrate`, `schema`) and `rok-gen` (`init`,
  `gen`, `generate`, `sqlc`). Each lists only its commands and refuses
  the other's with the binary that answers them (`rok-db gen`: "it is
  `rok-gen gen` (or `rok gen`)", exit 2). `rok_data_cli::Group`,
  `run_group` and `group_command` are the library side; `run` and
  `command()` are unchanged.

**Changed**

- Binaries and the installers are rok-v2's (workspace ADR 0032): its
  `rok` 0.7.0 links this CLI and `rok-tools-cli`, and its releases carry
  the `rok` archives, `install.sh` and `install.ps1`. This repository's
  release publishes the crates only, and its installers are gone;
  `rok-data`, the CLI on its own, is `cargo install rok-data-cli`.
- `rok tools …` is rok-v2's too: the CLI's `rok_tools` cfg, which wired
  rok-tools in once it published, is removed. `rok.toml`'s `[tools]`
  section stays readable here and belongs to rok-tools.

## [0.6.1] — the engine's package ships its client runtimes

**Fixed**

- `rok-data-sqlc`'s package ships `src/clients/dart/runtime.dart` and
  `src/clients/rust/runtime.rs.in`, which it embeds: the 0.6.0 release
  published `rok-data-macros`, `rok-data-core`, `rok-data` and
  `rok-data-codegen`, then stopped at `rok-data-sqlc`, so
  `rok-data-sqlc` and `rok-data-cli` were never published at 0.6.0. All
  six crates publish at 0.6.1; nothing else changes from 0.6.0.
- A test checks that every file the engine embeds is in its package.

## [0.6.0] — the toolchain split: `rok-data`, `rok-templates`, and the per-user servers

**Changed**

- The CLI's binary is `rok-data` (was `rok`): the `rok` command moves to
  [rok-v2](https://github.com/ateeq1999/rok-v2), which links this CLI
  (workspace ADR 0032). Help and usage errors show the name the CLI was
  called by; every command and flag is unchanged. The release archives
  and `install.sh` / `install.ps1` install `rok-data` until rok-v2 ships
  its own.
- The templates have their own repository,
  [rok-templates](https://github.com/ateeq1999/rok-templates), history
  included (workspace ADR 0032); the engine takes `rok-templates` 0.6
  from crates.io. Their gates (each template generated, compiled and
  tested; the monorepo's web, mobile, desktop and Dart Apps) moved with
  them. The tests here that check what `init` copies read the embedded
  crate instead of the files on disk.

**Added**

- `rok_data_cli::command()`: the whole command tree with its help, for a
  CLI that links this one (rok-v2's `rok`).

- Per-user server registry (`servers.toml`) in the OS config directory
  (`$XDG_CONFIG_HOME/rok`, `~/Library/Application Support/rok`, `%APPDATA%\rok`),
  with `ServerConfig { name, host, port, user, database, sslmode }` schema,
  atomic writes, 0600 permissions, and `deny_unknown_fields` parsing.
  New library API in `rok_data_cli::config`:
  `config_dir()`, `servers_path()`, `read_servers()`, `write_servers()`,
  `ServerConfig`, `ServersConfig` (ticket 01, ADR 0028).

- OS keychain integration for server passwords (ticket 02, ADR 0028).
  Uses the `keyring` crate (macOS Keychain, Windows Credential Manager,
  Linux `libsecret` via `secret-service`). Service name: `rok`.
  Account name: the server name.
  Fallback: environment variable `ROK_SERVER_<NAME>_PASSWORD`
  (uppercase, hyphens → underscores) when no keychain is available (CI/headless).
  `ROK_KEYCHAIN=off` skips the keychain entirely, so passwords come only from
  that variable; `ROK_CONFIG_DIR` overrides where `servers.toml` lives.

- Command group `rok server add|list|remove` for managing the server registry (ticket 03, ADR 0028).
  - `rok server add <NAME> [--url <URL>] [--host <HOST>] [--port <PORT>] [--user <USER>] [--database <DB>] [--sslmode <MODE>] [--password-prompt] [--json]`
    Registers a server; password from URL or prompt stored in keychain (or env var fallback).
  - `rok server list [--json]`
    Lists registered servers with password status (keychain ✓ / env ✓ / missing ✗).
  - `rok server remove <NAME> [--yes] [--json]`
    Removes server and its password; `--yes` skips confirmation prompt.
  All three commands support `--json` for machine-readable output.

- `rok db create|drop` with `--server` support (ticket 04, ADR 0028).
  - `rok db create <DBNAME> [--database-url <URL>] [--server <NAME>] [--json]`
    Creates a database on the target server. Connection resolution: `--database-url` > `--server` > `DATABASE_URL`.
  - `rok db drop <DBNAME> [--database-url <URL>] [--server <NAME>] [--yes] [--force] [--json]`
    Drops a database on the target server. Typed-name confirmation required (`--yes` bypasses for scripts).
    Refuses to drop the project's own database (from `DATABASE_URL`) unless `--force` is given.
  - Both commands support `--json` for machine-readable output.
  - SQL identifiers are quoted safely using double quotes.

- `rok db query <SQL> [--database-url <URL>] [--server <NAME>] [--json]`
  (ticket 05, ADR 0028): runs the statements of `SQL` in one transaction
  that commits only when every one succeeds; the first failure rolls back
  (DDL included) and exits 1 with PostgreSQL's message. A `;` inside a
  string, quoted identifier, dollar quote or comment does not split a
  statement; `COMMIT`, `BEGIN`, `ROLLBACK` and the other statements that
  would end the transaction early are refused. Result sets print as
  aligned tables, or with `--json` as
  `{"statements", "committed", "results": [{"columns", "rows", "row_count"} | {"rows_affected"}]}`
  (`{"committed": false, "error": {"statement", "message"}}` on failure).
  Same connection resolution as `create`/`drop`.

- The monorepo template (`rok init --template monorepo`) ships its web
  App at `apps/web`: the TanStack Start starter, run with plain bun, its
  dev server pointed at `apps/api` (`127.0.0.1:8080`). `init --template`
  now copies a template's binary files (the web App's `favicon.ico` and
  `logo.png`) byte for byte (0027 ticket 03, workspace ADR 0027).

- The monorepo template ships a desktop App at `apps/desktop`: rok-ui
  0.9.1's bloc-http starter (GPUI 0.2.2), a member of the root workspace
  (`cargo run -p desktop`), calling `apps/api` at `127.0.0.1:8080` unless
  `ROK_API_URL` says otherwise (desktop ticket 01, workspace ADR 0031).
  Its data layer is the generated `clients/rust` (a path dependency on
  `api-client`): the repositories sign in, read the signed-in account and
  page the users through it, a 422's broken rules land on form fields,
  and a 401 to the session's token expires the session. The starter's
  hand-written HTTP providers and its notes feature, which the API does
  not serve, are gone (desktop ticket 02). It signs up on the API's
  `POST /auth/register`, signs in (asking for the two-factor code when the
  account has one) and out, and keeps the session's token in the OS
  keychain (`keyring`, behind a `TokenStore` trait whose in-memory store
  the tests use), so it starts signed in; signing out or a 401 deletes
  the token (desktop ticket 03). The signed-in shell is rok-ui's
  `AdaptiveScaffold` with the users list (paged as the API pages it) and a
  user's detail, the Rok theme and a light/dark toggle (desktop ticket
  04). The monorepo's README and AGENTS.md say how to run, build and
  check it, verify an account and make one an admin for the users screens
  (desktop ticket 05, `docs/specs/desktop-starter.md`). Every signed-in
  user reaches a My account screen (their email, display name, verified
  state and role), where Home points; the users screens, `apps/api`'s
  admin routes, show in the navigation only for an admin, and anyone else
  opening `/users` is told it needs the admin role (the desktop review).

- The monorepo template's api App ships its contract as
  `apps/api/openapi.json`: the document it serves, written out by its
  `openapi` example (`cargo run -p axum-api --example openapi -- <PATH>`).
  The App's `tests/openapi_artifact.rs` fails when the file is not what it
  serves (0027 ticket 04, workspace ADR 0027).

- Generated Clients (0027 ticket 04, workspace ADR 0027, engine ADR 0027).
  A monorepo's `rok.toml` names its contract (`[contract]`: the
  `openapi.json` and the command that re-exports it) and its Clients
  (`[clients]`, `ts = "clients/ts"`). `rok gen` re-exports the contract
  after the Apps and emits the TypeScript client from it: typed calls per
  operation (`api.users.update(id, body)`), TanStack Query factories
  (`queries`, `mutations`, `keys`) and the web starter's
  `ApiError`/`FieldIssue`. `rok gen check` fails on a client out of date
  with the contract; `rok init --template monorepo` emits `clients/ts`.
  The contract is read as the OpenAPI 3.1 subset utoipa emits; anything
  else fails naming the construct.

- The generated Dart Client (0027 ticket 05, workspace ADR 0027, engine
  ADR 0027): `[clients]` `dart = "clients/dart"` has `rok gen` (and `rok
  init --template monorepo`) emit the pub package `api_client` for a
  Flutter App: `ApiClient(baseUrl:, session:)` with typed methods per
  operation (`api.users.list(page: 2, sort: UserSort.email)`), model
  classes, sealed classes for a `oneOf`, a `Session` hook for the bearer
  token, and `ApiException` carrying the status and a `422`'s field
  issues. It passes `dart analyze --fatal-infos`; the `dart_client` gate
  runs its tests where `dart` is installed.

- The monorepo's web App calls the API through the generated
  `clients/ts` (`@/lib/client` connects it to the session); the starter's
  `lib/http.ts` and `features/*/api.ts` are gone. Its features whose
  endpoints `apps/api` does not serve are hidden until it does: passkeys,
  QR sign-in, Google/GitHub sign-in and connected accounts, and creating a
  user with a password (0027 ticket 04).
- The monorepo template ships its mobile App at `apps/mobile`: the Flutter
  starter (`ateeq1999/Nestjs-Better-Auth-Flutter-Starter`), its data layer
  ported onto the generated `clients/dart` (sign-in with the two-factor
  step, sign-up, password reset, email verification, profile and photo,
  two-factor settings); its NestJS/Better-Auth HTTP code is gone. What
  `apps/api` does not serve is not in it: organizations, magic-link
  sign-in, the admin panel, deleting the account, OAuth and push
  notifications (a written FCM recipe). Firebase configuration ships as a
  placeholder. Its settings are `--dart-define`s, `API_URL` defaulting to
  `apps/api` on `127.0.0.1:8080`. The embedding leaves out a Flutter
  App's tool state and build output (`.dart_tool`, `build`, platform
  caches, `local.properties`, keystores); the `mobile_app` gate runs
  `flutter analyze` and `flutter test` on a scaffold where Flutter is
  installed (0027 ticket 06).
- The generated Rust client (0027 ticket 07, workspace ADR 0027, engine
  ADR 0027): `[clients] rust = "clients/rust"` has `rok gen` (and `rok init
  --template monorepo`) emit an async `reqwest` crate's sources from the
  contract: serde types of its own, one call per operation grouped by tag
  (`client.users().update(id, &body)`), the session's bearer token and a
  rejected-token handler, and one `ApiError`. Its `Cargo.toml` is the
  project's. In the monorepo template, `clients/rust` (package
  `api-client`) is a root workspace member, and the api App's
  `tests/client/` drive the flows of its other suites through it over a
  real socket.
- The API templates document `GET /api/v1/media/{id}/content`'s `200` as
  bytes (`application/octet-stream`), so a client generated from the
  document reads it as a file.

**Changed**

- The API templates' account routes answer `AccountResponse`
  (`/auth/me`, `/users/me`, the profile photo) and `NewAccountResponse`
  (sign-up, invitations) instead of their own `UserResponse`: utoipa kept
  one schema per name, so `GET /users/{id}` documented the account shape
  and a column added to `users` went missing from the contract and the
  generated Clients. `tests/openapi_schemas.rs` in each template, and
  rok-templates' `schema_names` test, fail when two schemas collide; the
  Rust and Dart Clients type an inline object as the component it equals
  before one of the same shape (0027 review).
- The monorepo's mobile App pins every dependency to the version its
  `pubspec.lock` resolves, and its gate installs with `flutter pub get
  --enforce-lockfile` (workspace ADR 0027; 0027 review).
- `rok server add --password-prompt`'s help no longer claims hidden
  input: the password is read from standard input as typed.
- `rok server add` without a password (none in `--url`, no
  `--password-prompt`) adds the server and warns that its password comes
  from `ROK_SERVER_<NAME>_PASSWORD`, as ADR 0028's spec shows, instead of
  failing.
- The confirmation prompts of `rok db drop` and `rok server remove` (and
  `--password-prompt`'s) go to stderr, so stdout holds only the result —
  one JSON document under `--json`.
- `rok server remove` with `ROK_KEYCHAIN=off` no longer reports a keychain
  entry as deleted; it says to unset the variable.
- `rok db create` with neither `--server` nor `--database-url` warns that
  it creates on the server `DATABASE_URL` names.
- The documentation site (`site/`) takes the monorepo template's web App
  design system in place of its own cyan/teal theme: the template's tokens
  (copied from `apps/web/src/styles.css`, with a test that fails when the
  copy drifts), its bundled Outfit and Oxanium fonts, and its Button, Badge
  and Card components.
- `rok init --template monorepo` prints the monorepo's next steps (`rok
  gen` from the root, the api App by its package name, the web and mobile
  Apps), the ones `rok sqlc init --template monorepo` prints, instead of a
  single crate's `src/db/` steps; both now name `clients/rust` too.
  They now name the desktop App (`cargo run -p <project>-desktop`), and
  every template's steps name `TOTP_ENCRYPTION_KEY` beside
  `DATABASE_URL` and `JWT_SECRET`: the API does not start without it
  (desktop ticket 05).
- The monorepo's README runs `rok gen` right after `rok init`: the
  scaffold leaves `apps/api`'s generated data layer and features to it,
  so `cargo build -p <project>-api` failed (140 errors) when the README
  was followed as written. A `rok-templates` test keeps it first (desktop
  ticket 05).
- `rok gen --migration <name>` in a monorepo stages each App's pair in
  the App's own `migrations/` when its settings leave `migrations` unset;
  it wrote them to the root's, as a first migration of the whole schema.
- `rok init --help` names `schema/` as the directory `--schema` seeds.
- The monorepo's `apps/api` is checked to be the `axum` template file for
  file (`rok-templates`' `tests/template_drift.rs`), and two doc comments
  that had drifted from it are the template's again.
- The templates' guide gains the monorepo template's page (its Apps, the
  one contract, the clients, the `rok gen` loop, what is hidden and why)
  and the push-notifications (FCM) recipe, moved from the mobile App's
  docs; getting started and the CLI reference walk the monorepo path.

## [0.5.0] — production-ready templates

Not breaking. The runtime and the generated API are unchanged. The
actix and axum templates (`rok sqlc init --template`) move to a crate of
their own and gain what production asks for: jobs and schedules on
PostgreSQL, logs, traces and metrics, a security baseline, draining on
`SIGTERM`, a Docker image, a local stack, CI, and an AWS deployment in
Terraform. Each capability is compiled in and off until its environment
variable is set. The templates' guide is
`crates/rok-templates/docs/guide/`. A project made from an earlier
template keeps working; `docs/migration-guide.md` §To 0.5.0 says what to
copy in for each capability.

**Changed**

- The actix and axum templates moved from `crates/rok-data-sqlc/templates/`
  to a crate of their own, `rok-templates` (workspace ADR 0022), which
  `rok-data-sqlc` depends on. `rok sqlc init --template` copies the same
  files as before. The engine's plain-init scaffold moved from
  `src/init/templates/` to `src/init/scaffold/`.
- `rok sqlc check-arch` treats `src/app/` as `app.rs` split up: its files
  may use the framework and `sqlx` as `app.rs` does, and it is no longer
  an unknown top-level directory. Rules 1 and 2 now read "`app.rs` or
  `app/`"; regenerate a project's `AGENTS.md` rules section from
  `rok sqlc init` output if you keep it in step.
- Both templates' wiring is split by responsibility into `src/app/`:
  `state.rs`, `router.rs`, `run.rs` and `ports.rs` (the last shared by
  both templates). The public paths (`app::AppState`, `app::run`, …) are
  unchanged.
- The templates' `Config::from_env` reports every wrong setting at once
  (`ConfigErrors`) instead of stopping at the first, and the server logs
  its settings at start-up with secrets left out (`Config::summary`).
  The config unit tests moved to `tests/config.rs`.

**Added**

- The templates' security baseline (`src/app/middleware.rs` over the
  shared `src/shared/security/headers.rs`): security headers on every
  answer (`nosniff`, `X-Frame-Options`, `frame-ancestors 'none'`,
  `Referrer-Policy`, and HSTS when `APP_URL` is `https://`);
  `CORS_ALLOWED_ORIGINS` (`*` or a list; unset, the API's own origin
  only); a panicking handler answered `500 internal_error` instead of a
  dropped connection; `GLOBAL_RATE_LIMIT`, a per-IP limit on every route
  but health (off by default); `JWT_SECRET_PREVIOUS`, to rotate
  `JWT_SECRET` without signing anyone out; and `deny.toml`, a
  `cargo deny check` policy that both templates pass. It ignores eight
  advisories no update can clear yet, each with the reason it can't reach
  the template (HS256-only JWTs, a build-time `zip`, the configured S3
  store, actix's TLS-only HTTP/2). Both templates' `Cargo.lock` files were
  refreshed: complete, and on `tokio` 1.54.1 (the yanked 1.54.0 had an
  advisory).
- The templates' logs and traces (`src/shared/telemetry.rs`, shared):
  `LOG_FORMAT=pretty|json`; every request in a span with its method,
  matched route, request id (answered as `x-request-id`, new in the actix
  template) and the caller's `user_id`; and, when
  `OTEL_EXPORTER_OTLP_ENDPOINT` is set, every span exported over
  OTLP/HTTP (OpenTelemetry 0.32), continuing a caller's W3C `traceparent`.
  `config::log_settings` replaces `config::log_filter`. The actix template
  traces requests through `tracing-actix-web`.
- The templates' metrics (`src/shared/metrics.rs`, shared): with
  `METRICS_ADDR` set, `/metrics` in Prometheus's format on a listener of
  its own: request counts and durations by method, matched route and
  status, the database pool's connections, and sign-ins by outcome and
  lockouts. Unset, nothing is recorded. The settings readers and their
  errors moved from `config.rs` to `src/shared/settings.rs`
  (`config::{ConfigError, ConfigErrors}` still name them).
- The templates' job queue (`src/shared/jobs/`, shared; workspace ADR
  0024): one `jobs` table on PostgreSQL. A typed `Job` (kind, serde
  payload, `run`, `MAX_ATTEMPTS`, `TIMEOUT`, `on_dead`) is enqueued on the
  flow's transaction (`Queue::enqueue`, `enqueue_with` for a delay or a
  unique key), and a `Worker` (`JOBS_CONCURRENCY`, 4) claims due jobs with
  `FOR UPDATE SKIP LOCKED`, woken in-process and by `NOTIFY`, retrying with
  backoff until a job is `dead`. A job's span continues the trace that
  enqueued it; `/metrics` counts `jobs_total` and `jobs_queued`.
  `Worker::run_due` runs due jobs inline for tests. Register kinds in
  `app::worker`.
- Mail is the `mail.send` job: the `outbound_mail` table is gone
  (template migration `0007_jobs`), `Sender` gives way to the worker, and
  `Outbox::enqueue`/`commit` keep their signatures, so no flow changes.
  `queries/mail.sql` is now `queries/queue.sql`.
- The templates' schedules (`src/shared/jobs/schedule.rs`):
  `Schedule::new().cron::<Kind>("*/5 * * * *")?` enqueues a job kind at
  every tick of a UTC cron pattern (`croner`), once across every process:
  the transaction that moves the kind's `last_tick` forward
  (`job_schedules`, template migration `0008_job_schedules`) enqueues it.
  A missed tick runs once; a new schedule waits for its first tick; a bad
  pattern stops the start-up. `Schedule::tick_at` takes the clock for
  tests. The upload sweep is the scheduled `media.sweep_uploads` job
  (every 5 minutes); `MEDIA_UPLOAD_SWEEP_SECS` and
  `MediaService::sweep_every` are gone.
- `APP_ROLE=all|web|worker`: `web` serves the API only, `worker` runs the
  jobs and the schedule and answers only `/health`, `all` (the default)
  both. The axum template's `app::serve_as(role, …)` serves one role;
  `app::serve` is `all`. Kinds are registered in `app::worker`, schedules
  in `app::schedule`.
- The templates' `jobs` feature, the job queue for admins (`AdminUser`):
  `GET /api/v1/jobs` (by `status` and `kind`, paginated, payloads never
  answered), `POST /api/v1/jobs/{id}/retry` (a dead job back in the queue)
  and `DELETE /api/v1/jobs/{id}` (cancel; `409` while a worker runs it).
  Retries and cancellations are audited (`job.retried`, `job.cancelled`)
  through the feature's `JobAudit` port, implemented in `app/ports.rs`.
  Its SQL is `queries/jobs.sql`.
- The templates drain on `SIGTERM` (`src/shared/shutdown.rs`, shared):
  `/health/ready` answers `503` at once while every request is still
  served for `SHUTDOWN_DELAY_SECS` (5), then the server stops accepting
  and the requests in flight finish within `SHUTDOWN_GRACE_SECS` (25),
  then the job worker stops; a job still running past the grace is
  abandoned, its claim released, no attempt counted
  (`Background::stop_within`). The actix template handles the signal
  itself (`disable_signals`, `shutdown_timeout`) so the readiness flip
  comes first. The axum template's health routes are `app::health_routes`.
- The templates ship how they run and ship (shared by both): a
  `Dockerfile` (cargo-chef layers, distroless, non-root, JSON logs; the
  binary found whatever the crate is named), `compose.yaml` (PostgreSQL,
  Floci 2.3.0 for S3 with its bucket created, Mailpit; the
  `observability` profile adds Jaeger, Prometheus and Grafana with an
  *API* dashboard under `deploy/compose/`), and
  `.github/workflows/ci.yml` (fmt, clippy, tests against PostgreSQL,
  `rok sqlc g --check` and `check-arch`, `cargo deny`, the image build).
- The templates' AWS reference deployment (`deploy/terraform/`, shared;
  workspace ADR 0025): modules for the network, RDS PostgreSQL 17, the
  uploads bucket, SES (identity, DKIM, SMTP credentials), Secrets Manager
  (`JWT_SECRET` and `TOTP_ENCRYPTION_KEY` generated) and the service (ECR,
  ECS Fargate `web` behind an ALB checking `/health/ready` and `worker`,
  a `migrate` task, CloudWatch logs), composed by `modules/app` for
  `envs/staging` and `envs/prod` (S3 state with `use_lockfile`, made by
  `bootstrap/`). `envs/local` applies what Floci emulates, and
  `terraform test` checks it; CI runs it beside `validate` and `tflint`.
  The `Dockerfile` gains a `migrate` target (`rok migrate up` over
  `migrations/`), which the deploy runs before each version.
- The actix template's application is `app::app(state)`, the routes of
  `app::configure` wrapped in that middleware; tests build it with
  `test::init_service(app(state))`. It depends on `actix-cors` and
  `futures-util` now.
- The templates' guide, `crates/rok-templates/docs/guide/`. It has pages
  on jobs and schedules, observability, security and deploying to AWS,
  and recipes for Sentry, OTLP metrics, a shared rate limit on Redis,
  signed outbound webhooks and OIDC sign-in. `auth.md` and `files.md`
  moved there from the engine's guide; the engine's pages link to them.
  `tests/guides.rs` checks each sample marked `<!-- source: … -->`
  against the template file it quotes. The templates' READMEs open with
  a pointer to the guide.

## [0.4.0] — typed keys, a runtime-backed data layer, and direct uploads

Breaking. Model lookups take the model's own key type, and the code
`rok sqlc` generates calls the runtime instead of sqlx. Follow
`docs/migration-guide.md` §To 0.4.0: regenerate, then fix what the
compiler reports.

**What changed**

- **Lookups take the model's own key type** (runtime). `Model` gains
  `type Key`, set by `#[derive(Model)]`:
  - for a single-column key, the key field's type;
  - for a composite key, a tuple of the key fields' types, in field order;
  - for a table without a key, `()`.

  `find`, `find_or_fail`, `find_many`, the `ModelCollection` key lookups
  and `audit::history` take `impl Into<Self::Key>`, so
  `Product::find(&db, category_id)` no longer compiles. A composite key of
  the wrong length used to fail at run time (`InvalidQuery`); now it fails
  to compile.
  - **Breaking for a hand-written `impl Model`:** add a `type Key` line.
- **Generated data calls go through the runtime** (engine ADR 0023).
  - **Errors:** generated functions fail with `rok_data::Error` instead of
    `sqlx::Error`. Annotated queries run through `rok_data::raw`.
  - **Reads:** generated reads are the runtime's verbs (`Row::find`,
    `Row::all`, `Row::count`) plus chains built from the schema:
    `Row::by_primary_key(…)` and `Row::by_<columns>(…)`, which replaces
    `get_by_<columns>`.
  - **Lists and search:** `Row::matching(…)` and `Row::search(filter)`
    return a chain that the caller orders and pages, so a page and its total
    take one round trip.
  - **Removed:** `get_by_id`, `exists_by_id`, `list`, `list_page`, `count`,
    `search_after` and the deprecated `write::` wrappers.
  - **Composite key structs** convert into the model's key tuple.
  - **Still on sqlx:** two generated paths, the versioned `patch` and the
    relationship loaders. A later release converts them.

  The runtime gains `Raw::scalar_optional`, `Raw::scalars`, `&[T]` binds
  for enum and typed-id lists, `NaiveDateTime`/`NaiveTime` list binds, and
  `Column::icontains`. The engine's CHANGELOG lists every changed
  generated item.
- **Direct uploads in both templates** (engine ADR 0024).
  - **Uploads:** `POST /media/uploads` answers a presigned `PUT` URL.
    `POST /media/{id}/complete` then checks the stored bytes' size and
    detected type.
  - **Downloads:** `GET /media/{id}/url` answers a presigned download URL.
  - **Signing:** `s3://` stores sign the URLs themselves (SigV4).
    `file://` and `memory://` URLs are signed by the API, under a key
    derived from `JWT_SECRET`.
  - **Cleanup:** a background sweep deletes uploads that are never
    completed.
  - **Breaking for a project made from a template:** migration
    `0006_media_uploads` is new, and so are the settings
    `MEDIA_UPLOAD_URL_TTL_SECS`, `MEDIA_DOWNLOAD_URL_TTL_SECS`,
    `MEDIA_UPLOAD_SWEEP_SECS` and `STORAGE_PUBLIC_ENDPOINT`.
    `shared/storage/` is scaffolded once, so an existing project copies the
    change in by hand.

**Added**

- **`rok sqlc check --db`** (engine ADR 0026). It prepares every annotated
  query against the database without running it, and reports each type,
  name or nullability mismatch with the generated code as
  `file:line: Query: message`. Without `--db`, `rok sqlc check` fails on
  drift, as `generate --check` does.
- **`rok sqlc generate --watch`** regenerates whenever the schema, the query
  files or the config file change.
- **`rok sqlc query add <NAME> <SQL> --file <stem>`** appends one annotated
  query and generates. The file keeps the query only if it generates.
- **`rok_data::introspect::describe(db, sql)`** answers a statement's
  parameter and column types without running it.
- **`db::prelude`** brings in the generated search vocabulary too, so one
  `use` serves a handler.

## [0.3.0] — the model-shaped data layer

Breaking. The data layer's surface becomes the model's: the typed verbs
live on the model, not in generated module namespaces, and hand-written
models speak the same dialect the generated ones do
(`docs/specs/0.3.0-model-shaped-data-layer.md`, workspace ADR 0014 /
engine ADR 0018). Follow `docs/migration-guide.md` §To 0.3.0; the
compiler finds the call sites.

**What changed**

- **The actix template has accounts** (rok-sqlc 0.6, absorbed; engine ADR
  0020). `rok sqlc init --template actix` now makes an API with sign-up,
  sign-in, email verification, password reset and change, email change
  and invitations. These are hand-written features, with a mail outbox,
  a `/auth` rate limit and a per-request account check.
  - **Breaking for a project made from the template:** `JWT_SECRET` must
    be at least 32 bytes, there is a new schema file and migration
    (`0002_auth`), and `app::configure` takes the `AppState`.
  - **Rule:** `rok sqlc check-arch` gains `cross-feature-query`.
  - **The axum template** has the same accounts (rok-sqlc 0.7). Its
    framework-independent files are byte-identical to actix's, which
    `template_drift` checks. `rok init -w axum` (and `rok sqlc init -w
    axum`) scaffolds the skeleton for axum.
  - **Both templates gain two-factor sign-in, API keys, uploads and
    profile photos** (rok-sqlc 0.8, engine ADR 0022). New required
    setting `TOTP_ENCRYPTION_KEY`; uploads are stored where `STORAGE_URL`
    says (`file://`, `s3://`, `memory://`); a disallowed upload type
    answers `415 unsupported_media_type`. Three new migrations
    (`0003_two_factor`, `0004_api_keys`, `0005_avatars`), and new
    dependencies in the templates' own manifests only. See the engine's
    [Files and storage](crates/rok-data-sqlc/docs/guide/files.md).
  - **Guide:** see the engine's
    [Accounts and sign-in](crates/rok-data-sqlc/docs/guide/auth.md).
- **Generated writes answer a missing reference or a taken value as a
  field's `422`** (rok-sqlc 0.5, absorbed; engine ADR 0019). Before, they
  answered a `409 conflict` naming a constraint. A create or update checks
  `exists` (each foreign key names a row the caller can see) and `unique`
  (no other row holds a unique key's values, ignoring case for a
  `lower(<column>)` index) before it writes. The `409` stays for races.
  `[api.rules]` adds string rules by VineJS name (`email`, `url`,
  `minLength(3)`, `trim`, …). See the engine's CHANGELOG.
- **Generated lists can page by cursor** (dx-first ticket 16):
  `[api] cursor = ["events"]` makes a table's `GET` list take
  `?cursor=`. Each page answers the cursors of the pages on either side,
  in the `pagination` shape (`{items, next, prev, per_page}` when
  compact). Pages stay stable while rows are added. A cursor the list
  didn't give out is a `400`. The run refuses a listed table without a
  primary key, or with a nullable sort key.
- **Keyset cursors carry typed keys**: a sort key or primary key whose
  type is a newtype over a built-in one (the generated `BookId`, any
  `impl_value!` / `DbNewtype` over `BIGINT`, `UUID`, `TEXT`, …) now
  travels in a cursor as its plain value. Before, the cursor of a
  generated model's page failed to parse, so `cursor_paginate` couldn't
  go past the first page.
- **Generated list endpoints answer in the page shape you pick**
  (dx-first ticket 15): `[api] pagination = "compact"` (the default,
  unchanged `{items, page, per_page, total}`), `"paginator"` (the flat
  paginator object with `current_page`, `last_page`, `from`/`to` and a URL
  per neighbouring page) or `"resource"` (`{data, links, meta}`). The
  links come from the request's URI, so every other query parameter is
  kept; the OpenAPI document describes the chosen shape.
- **The write verbs live on the generated model** (dx-first ticket 03,
  as 0.3.0 spec D5 and engine ADR 0018 promised): `Book::create(&db,
  input)`, `Book::patch(&db, id, patch)`, `Book::destroy(&db, id)`, and
  `restore` / `delete_permanently` / `attach` / `detach` / `sync` where
  they apply. The module functions (`books::write::create`) remain for one
  release as `#[deprecated]` wrappers that forward to the model verb.
  Generated services call the model verbs. A `[generate.scopes]` entry
  named like one of these verbs is rejected (it would collide).
- **Generated code reads like hand-written code** (dx-first ticket 03):
  chains start at the model (`Book::filter(Book::ID.eq(id))` instead of
  `Book::query().filter(…)`), and a row carries only the attributes the
  derive cannot infer (0.3.0 spec D4) — no `#[rok(table = …)]` when the
  table is the struct's pluralized snake_case name, no
  `#[rok(primary_key)]` on a lone `id` key. Schema-qualified tables and
  other keys keep theirs.
- **Every documented sample compiles** (dx-first ticket 14): the Rust
  samples of `docs/getting-started.md`, `docs/migration-guide.md` and the
  new `docs/pagination.md` each name the compiled file that holds them
  (`<!-- source: … -->`), and a test fails when one drifts; a migration's
  0.2 "before" says why it is not compiled. Fixing them fixed the samples
  that did not compile (a relation called on an `Option`, a scope on a
  model without it, placeholder scope paths). The generated
  `db::prelude` now also brings each model's inputs, ids and keys
  (`NewBook`, `UpdateBook`, `BookId`), and the runtime prelude brings
  `Cursor` and `CursorPage`.
- **Pages on the wire** (dx-first ticket 13, the runtime half): with the
  `serde` feature, `Page`, `SimplePage` and `CursorPage` answer as the
  paginator object (`page.paginator(links)`: `data`, `current_page`,
  `last_page`, `from`, `to`, the `*_page_url`s, …) or in the resource
  envelope (`page.resource(links)`: `{data, links, meta}`),
  with `rok_data::wire::Links::from_request(uri)` building every link from
  the request — the other query parameters kept, the page parameter
  nameable (`page_param`), an optional `fragment`. Each type's own
  `Serialize` stays the compact shape. `CursorPage` carries its
  `per_page`. Generated endpoints choosing a shape and generated cursor
  lists follow (tickets 15 and 16).
- **Walking a large table** (dx-first ticket 12): `chunk` reads one
  `LIMIT`/`OFFSET` window per query (ordered by the primary key when the
  query has no order) instead of one streamed query, so no connection is
  held while the callback runs — a callback that writes back through the
  same pool works with a pool of one, where it used to time out. New:
  `chunk_by_id` (keyset windows by primary key, correct when the callback
  deletes or updates its rows), `lazy` / `lazy_by_id` / `lazy_by_id_desc`
  (a `Stream` of rows read in windows), `each` / `each_by_id` (one row per
  call). A callback returns `Ok(())` to go on, or `Ok(false)` /
  `ControlFlow::Break` to stop (the `Walk` trait). Every walk takes a
  session — the database, a pool, a transaction or a connection.
- **Cursor pagination in both directions** (dx-first ticket 11): a
  `CursorPage` carries `prev` beside `next` (`prev_cursor()`,
  `next_cursor()`, `has_prev()`, `has_more()`), and passing a page's
  `prev` to `cursor_paginate` returns the page before it, rows in the
  query's order — for plain column orders and for the wrapped keyset of
  joins and expression orders alike. A cursor records its direction
  (`Cursor::before`, `is_backward()`); a backward token starts with `-`,
  and forward tokens are byte-for-byte those of 0.2. The JSON of a
  `CursorPage` gains `"prev"`. The size argument is named `per_page`.
- **Offset pagination, complete** (dx-first ticket 10):
  `simple_paginate(&db, page, per_page)` returns a `SimplePage` (the rows
  and `has_more`, one query, no `COUNT(*)`); `page(&db, n)` paginates at
  the model's size — `#[rok(per_page = 25)]`, 15 by default
  (`Model::PER_PAGE`); `for_page(page, per_page)` is the window as chain
  sugar; `.with(..).paginate(..)` returns a `LoadedPage` (the page's
  numbers beside the loaded includes); relation chains
  (`user.posts().paginate(..)`) paginate like any chain. `Page` gains
  `last_page`, `first_item` / `last_item`, `on_first_page` /
  `on_last_page`, `has_pages` and `into_collection`.
- **One pagination vocabulary** (dx-first ticket 09): "page" means a
  page of rows. The generated limit/offset type `db::search::Page` is now
  `db::search::Window` (`Page` stays a deprecated alias for one release),
  and a runtime `rok_data::Page<T>` converts to the wire's
  `PaginatedResponse<T>` in one call — a hand-written handler can answer
  `PaginatedResponse::from(Book::query().paginate(&db, page, per_page)
  .await?)`. The wire JSON and OpenAPI schemas are unchanged.
- **Write API polish** (dx-first ticket 08): `update_only` takes any list
  or iterator of columns — `user.update_only(&db, [User::NAME])`, the
  shape `upsert_on` already takes, instead of `Some(vec![…])` — and an
  empty list is an `Error::InvalidQuery`, never a silent full `update`.
  The hidden `delete_by_key` is no longer a trait method callable on every
  model.
- **Model collections**: `Select::get` returns a `ModelCollection<M>`
  (`all` keeps the plain `Vec`), so `User::query().get(&db).await?.find(1)`
  needs no conversion. Through `Deref` it is a `Vec`; its verbs work by
  primary key — `find`/`find_or_fail`, `contains_key`, `model_keys` (each
  row's whole key, composite keys included), `unique`, `only`/`except`,
  `intersect`/`diff`, `partition` — and go back to the database:
  `to_query`, `fresh`, `load`/`load_missing`. It serializes as the array of
  its rows (`to_json`, `Serialize`); a JSON failure is `Error::Json`
  (`is_json()`). Build one from a `Vec`, an array or an iterator (0.4.0
  ticket 01, dx-first ticket 07).
- **The API layer keeps its 0.2 names** (dx-first ticket 05): the
  generated handlers and service methods (`update`, `remove`,
  `remove_permanently`, `Remove{Row}Query`), the OpenAPI `operationId`s,
  the `[api.skip]` names and `ApiEndpoint::Remove` are those of 0.2.0;
  only the data layer's verbs take the new names. A `service.rs` override
  and an OpenAPI client keyed on `operationId` keep working.
- **One "not found" convention** (dx-first ticket 04): every verb that
  targets one row by key fails with `Error::NotFound` when no row has it.
  The runtime adds `Update::returning_one_or_fail` and makes
  `Error::not_found::<M>(key)` public; generated services answer the
  same 404 as before through a generated `or_not_found` helper.
- **One import**: the generated `db/mod.rs` writes a `prelude` module.
  `use crate::db::prelude::*;` brings rok-data's prelude, every model,
  and each model's `{Model}Scopes` trait, so chained scopes no longer
  need a hand-written `use …::models::UserScopes` (dx-first ticket 02).
- **The write spine renaming**: the generated write
  verbs are `create` / `patch` / `destroy`; `patch` returns the patched row
  (`Result<User>`), `destroy` returns `Result<()>`, a missing key is
  `Error::NotFound` (dx-first ticket 04), and the soft-delete pair keeps `restore` / `delete_permanently`
  (ticket 01). `save` is removed, not aliased (its meaning is ambiguous);
  the row-level verbs are `insert`, `update`, `update_only`, `upsert`,
  `upsert_on`, and these move to a separate `ModelWrite` trait — `Model`
  is the read surface, `ModelWrite` the write surface, both in the
  prelude so `use rok_data::prelude::*;` keeps working (ticket 04).
- **Conventions are inferred, not declared**: a `created_at`
  /`updated_at` pair manages its timestamps without
  `#[rok(timestamps)]`, a `deleted_at` field soft-deletes by default, a
  single-column primary key is inferred from the id field and generated
  `id`s skip the create body — each with an explicit `no_*` opt-out when
  a model genuinely does not want the inference.
- **Relations on the model** (ticket 02): the schema's foreign keys
  become `#[rok(belongs_to = …)]` / `#[rok(has_many(posts = Post::AUTHOR_ID))]`
  on the generated rows, expanded by the derive into typed relation
  constants and accessors — `user.posts()` is a lazy chain, and
  `db::relationships::load_posts` stays the batched loader.
- **Named scopes** (ticket 03): `#[rok(scopes(popular = path::to::popular))]`
  gives the model a chainable entry (`User::popular()`) and the chain a
  same-named method (`User::popular().active()`), through a `UserScopes`
  trait the derive writes next to the model. A scope takes the chain by
  value and returns it (`fn popular(q: Select<User>) -> Select<User> {
  q.filter(…) }`, dx-first ticket 06); a scope named like a runtime verb
  fails the derive (a test keeps that list in step with the runtime). A
  scope may take parameters declared in the attribute
  (`#[rok(scopes(older_than(n: i32) = …))]`), and generated models get
  their scopes wired from the engine's new `[scopes]` configuration
  (one `#[rok(scopes(name = path))]` attribute per scope; the app owns
  the functions). `default_scope` and `.unscoped()` are unchanged.
- **Chain additions and spellings**: `Model::exists`, `Model::where_` /
  `Select::where_` (another name for `filter`; the raw
  identifier is the only form that compiles since `where` is a keyword),
  `Select::get` (another name for `all`), and `Select::chunk`
  for walking big tables in windows.
- **Chain sugar** (0.4.0 ticket 02), every condition typed on its column:
  - **Several columns, one condition:** `filter_any([User::NAME,
    User::EMAIL], |c| c.ilike(&pattern))`, `filter_all`, `filter_none`.
  - **Date parts:** `date()`, `time()`, `year()`, `month()`, `day()`,
    `hour()` on a column, compared like any value
    (`Post::PUBLISHED_AT.year().eq(2026)`).
  - **JSON:** `json_array_len()`, `json_array_len_at(path)`,
    `json_has_path(path)`, `json_contains_at(path, json)`.
  - **Two columns of a row:** `eq_column` … `lte_column`
    (`Task::UPDATED_AT.gt_column(Task::CREATED_AT)`); `NULL`-aware
    equality with `is_not_distinct_from` / `is_distinct_from`.
  - **Raw escapes:** `filter_raw`, `having_raw`, `order_by_raw`,
    `group_by_raw`, `select_raw`, each `?` bound in order.
  - **Composing:** `pipe(f)` hands the chain to a function and returns
    its result; `tap(f)` looks at it and goes on; `debug()` prints the
    statement and its parameters to stderr.
  - **`UNION`:** `select.union(other)` / `union_all(other)` return a
    `Union` with its own `order_by`, `limit`, `offset` and `all` / `get` /
    `first`.
  - **Counters:** `Update::increment_each([(Post::VIEWS, 1), (Post::SCORE,
    5)])`, in one `UPDATE`.
- **Rows by their relations** (0.4.0 ticket 03, first half):
  - **Filters:** `has(User::POSTS)`, `doesnt_have(…)`, `where_has(rel, |q|
    q.filter(…))`, `where_doesnt_have(…)` and `where_relation(rel,
    condition)`, as correlated `EXISTS` subqueries that keep the related
    model's scopes.
  - **Aggregates beside the rows:** `with_count(User::POSTS)` and
    `with_exists(User::PROFILE)` on a chain, `load_count` on a collection,
    read with `view.count(User::POSTS, user)` / `view.exists(…)`. Sums,
    averages, minimums and maximums are handles on the relation
    (`User::POSTS.sum::<i64>(Post::VIEWS)`, `avg`, `min`, `max`), loaded
    with `with` and read with `view.aggregate(handle, user)`. Each is one
    batched `GROUP BY` query; the value type picks the SQL cast.
  - **Through relations:** `#[rok(has_many_through(posts = Post::USER_ID,
    through = User::COUNTRY_ID))]` on `Country` gives `Country::POSTS` (a
    `HasManyThrough`) and `country.posts()`; `has_one_through(…)` the
    one-target form. They load with `with`, filter with `has` /
    `where_has`, and count with `with_count` like a direct relation; the
    intermediate model's scopes apply. Declared, not inferred from the
    schema.
  - **Many-to-many:** `#[rok(belongs_to_many(roles = Role, pivot =
    "role_user"))]` gives `User::ROLES` (a `BelongsToMany`; pivot columns
    default to `user_id` / `role_id`) and `user.roles()`, loading,
    filtering and counting like the others. It writes the pivot:
    `attach` / `attach_with` (extra pivot columns), `detach` /
    `detach_all`, `sync`, `sync_without_detaching`, `toggle` and
    `update_existing_pivot`; `sync` and `toggle` are one statement each.
    The generator writes the attribute on both sides of a join table that
    pairs two primary keys (generated models change: a `belongs_to_many`
    line and an import).
- **Scopes and loaded views** (0.4.0 ticket 04): `#[rok(default_scope =
  …)]` may repeat — every one applies, combined with `AND`, and
  `.unscoped()` drops them together. `Loaded::get(rel, row)` reads a
  relation like `try_get` but fails with the new `Error::NotLoaded`
  (`is_not_loaded()`) when the query didn't load it, its message naming
  `.with(…)`.
- **Row lifecycle verbs** (0.4.0 ticket 08), on `ModelWrite`:
  `row.first_or_insert(&db, [Tag::SLUG])` returns the row holding that
  unique key, inserting this one first when there is none, in one atomic
  statement; `row.increment(&db, Post::VIEWS, 1)`, `decrement` and
  `touch(&db)` update in the database without reading first and return
  the stored row. `update_or_create` is the existing `upsert_on`, `fresh`
  the existing `reload`.
- **Input validation** (input-validation ticket 02, workspace ADR 0021):
  `#[derive(Input)]` on any struct gives `input.validate(&db).await?`
  (and `validate_fields()` without a session), returning the input
  transformed or every failure as `Error::Validation`. Fields take the
  model's `#[rok(validate(…))]` rules and `#[rok(transform(trim,
  lowercase, uppercase, normalize_email, custom = path))]`, run before the
  rules; `#[rok(validate_with = path)]` checks the whole input. A
  transform on a model, or a key / timestamp / relation attribute on an
  input, fails to compile.
- **The rule catalogue in the runtime** (input-validation ticket 07,
  first part): `validate(url, uuid(versions = [4]), ulid, ip(version =
  4), alpha(spaces, dashes, underscores), alpha_numeric(…), ascii,
  one_of(values = [...]), not_one_of(…), starts_with = "…", ends_with =
  "…", hex_code, iban, jwt, credit_card(providers = [...]), coordinates)`
  on models and inputs, each answering the generated APIs' code
  (`alphaNumeric`, `ipAddress`, `in`, …) and message. The checks live in
  `validate::catalogue`, moved from the engine's generated
  `shared/validation.rs`. The `normalize_email` transform now normalizes
  as the generated APIs' `normalizeEmail` does (provider subaddresses,
  Gmail dots) and no longer trims: add `trim` before it.
- **Validation context** (input-validation ticket 08, first part):
  `#[rok(context = Caller)]` on an input lets its database rules read
  who is asking: `scope = fn(&Self, &Caller) -> Expr<…>`, custom rules
  `async fn(&T, Journey, &Caller)`, `validate_with_database` likewise;
  validate it with `input.validate_with(&db, &caller)` (`validate(&db)`
  stays for inputs without a context). `Input` gains `type Context`
  (`()` unless declared); nested inputs share their parent's.
- **Generated APIs answer with the runtime's validation errors**
  (input-validation ticket 08, second part). The generated
  `ValidatedJson`, `FieldFailures` and the scaffolded `AppError` carry
  `rok_data::ValidationErrors`, so a generated `422` and a hand-written
  `input.validate(&db)` answer the same body. Breaking on the wire: a
  generated failure's `params` no longer echo the rejected `value`.
  `rok_data::Error::Validation` now maps to a `422` with its `details`.
  The runtime's `serde` support is always on (the `serde` feature stays,
  empty, for compatibility). A project made from a template keeps
  compiling once its `error.rs` imports `rok_data::ValidationErrors` in
  place of `validator::ValidationErrors` (migration guide).
- **Generated validation is the runtime's `Input`** (input-validation
  ticket 08, third part; engine ADR 0025). Breaking for generated code and
  the templates:
  - **Derives:** `derive = ["validate"]` puts `rok_data::Input` on `NewX`
    and `UpdateX` in place of `validator::Validate`. The API's bodies
    derive it too.
  - **Rules:** a body's rules read `#[rok(transform(trim),
    validate(alpha_numeric(dashes), min_length = 3))]`, the runtime
    catalogue's names with VineJS codes. `dto.rs` has no generated check
    functions.
  - **Extractor:** `ValidatedJson<T>` takes any `T: rok_data::Input`
    without a context.
  - **Dependencies:** an app drops `validator` (and `regex`), and enables
    rok-data's `regex` feature for pattern rules.
  - **On the wire:** an exact length answers `params` `{"min": n, "max":
    n}`.
  - **`custom` on an `Option`:** a `custom` rule on an `Option` field now
    runs only on `Some`, with the value inside, as the other rules do.
- **Exclusive ranges and decimal ranges** (input-validation ticket 08):
  `range(exclusive_min = 0, max = 5)` leaves a bound out (`"must be
  greater than 0 and at most 5"`, JSON Schema `exclusiveMinimum`); a
  `Decimal` field takes `range` (feature `decimal`); an integer bound
  stays an integer in the rule's `params`. New: `validate::Bound`,
  `validate::bounded`.
- **Custom codes and patch transforms** (input-validation ticket 08):
  `custom(function = path, code = "immutable")` answers its own code
  (`custom = path` stays code `custom`); transforms apply through an
  `Option<Option<String>>` patch field.
- **The rest of the catalogue** (input-validation ticket 07, second
  part): `min_length = n`, `max_length = n`, `fixed_length = n`
  (characters, codes `minLength` / `maxLength` / `fixedLength`), and with
  the new runtime feature `regex`: `mobile(locales = [...], strict)`,
  `postal_code(countries = [...])` and `regex = "…"`. Their pattern
  tables live in `validate::patterns` and in the engine; a test keeps the
  two equal. Generated APIs now import the rule checks
  (`is_url`, `letters`, `normalize_email`, …) from
  `rok_data::validate::catalogue` instead of carrying copies in
  `shared/validation.rs`.
- **JSON Schema from an input** (input-validation ticket 06):
  `Input::json_schema()` (feature `json`) is the input as JSON Schema
  2020-12 — field types, `required`, and the field rules a schema can say
  (`minLength`/`maxLength` or `minItems`/`maxItems`, `minimum`/`maximum`,
  `format: email`); nested inputs inline, enum inputs as `oneOf`; a type
  it can't describe is `{}`. The mapping is documented on
  `validate::schema`.
- **Messages and the reporter** (input-validation ticket 05):
  - **Messages:** every rule takes `message = "{field} needs {min}+
    characters"` (`email(message = "…")` for the bare ones), quoting
    `{field}` and the rule's arguments. A `Messages` provider (rule code
    and field → template) applies through `errors.localized(&provider)`,
    leaving a rule's own message alone.
  - **Errors:** a `FieldError` carries its rule's arguments as
    `params: Vec<(&str, Value)>`.
  - **Reporter:** `ErrorReporter` turns errors into a body;
    `DetailsReporter` writes the generated APIs' `{"error": {"code":
    "validation_failed", "message", "details": {path: [{code, message,
    params}]}}}`.
  - **Breaking:** the `axum` responses of every `rok_data::Error` use the
    wire standard's `{"error": {"code", "message", "details"?}}` shape
    instead of `{"error": code, "message", "fields"?}`.
- **Input shapes** (input-validation ticket 04): `required_if(field =
  kind, equals = "company")`, `required_unless(…)` and
  `required_with(other)` (code `required`, presence by the `Presence`
  trait: `Some`, non-empty text or list); `#[rok(when = path)]` gates a
  field's rules; `#[rok(nested)]` validates an input field, an `Option`
  or a `Vec` of them with the outer one — transforms, rules and built-in
  database rules — at paths like `address.city` / `lines.2.sku`; an enum
  whose variants each wrap an input delegates to its variant.
- **Database rules on input** (input-validation ticket 03):
  `unique(User::EMAIL, except = id, case_insensitive, scope = path)` and
  `exists(Org::ID, scope = path)` in an input field's `validate(…)`, the
  target model's own scopes applying; `database = path` on a field and
  `#[rok(validate_with_database = path)]` on the input for checks of
  their own. `validate(&session)` runs them only on fields whose field
  rules passed, the built-in ones in one statement. On a
  `#[derive(Model)]`, a database rule fails to compile. New:
  `Column::eq_ignore_case`, `Journey::reborrow` (public).
- **Validation errors carry paths; violations read as field errors**
  (input-validation ticket 01, workspace ADR 0021). Breaking:
  `FieldError::field` is a `String` path (`address.city`, `items.2.sku`)
  and `FieldError` gains `params`. A unique or foreign-key violation on a
  row write is `Error::Validation` at the column, code `unique` /
  `exists`, instead of `Error::Database`; `is_unique_violation()` /
  `is_foreign_key_violation()` stay true. See the migration guide.
- **What a field type can't say** (0.4.0 ticket 05, narrowed by
  decision: a field's Rust type already is its cast):
  - **Encrypted columns:** `Encrypted` (feature `encryption`) stores
    `base64(nonce ‖ AES-256-GCM ciphertext)` in a `TEXT` column and gives
    the plain text back; the key comes from `set_encryption_key` or
    `ROK_DATA_ENCRYPTION_KEY`.
  - **Value objects:** `#[rok(value_object(address: Address = (street,
    city)))]` generates `address()` and `set_address(…)`.
  - **Write-time transforms:** `#[rok(set_with = hash)]` on a field
    generates `set_<field>(value)` storing `hash(value)`.
- **Nested eager loading** (0.4.0 ticket 10):
  `.with(Author::POSTS.with(Post::COMMENTS))` loads each level in one
  batched query, to any depth; `view.nested(Author::POSTS)` is the
  `Loaded<Post>` view the nested relations are read from (the nested
  target must be `Clone`). `Loaded::load(&db, includes)` loads (or
  reloads with other shaping) beside the existing `load_missing`. The
  relation-constant extension trait is `RelationExt` (aggregates and
  `with`).
- **`INSERT … SELECT`** (0.4.0 ticket 09): a projected query writes its
  rows into another table without fetching them —
  `Post::filter(…).select((Post::TITLE,)).insert_into::<Archive>([Archive::TITLE])`,
  then `exec` (the row count) or `returning` (the inserted rows).

**What does not change**: PostgreSQL 12+ (DATABASE_URL), the migration
system, `#[rok::test]`, the connection policy, `filter` staying the
canonical verb, and the two-lane principle (schema generates baseline
CRUD; query files carry application SQL).

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
reports the missing, changed and stale files, and fails on drift. An
un-staged truth change is a finding of its own there — the prose names
the last staged migration and the version the pair would take
(`0002_pending`), and `--json` carries it in `pending_migration`; no
file that does not exist is reported among the missing ones, and the
run still fails. The settings file is the unified `rok.toml`
(workspace ADR 0011), whose `[generate]` section holds the generation
settings; the engine's `rok-data-sqlc.toml` stays readable. The legacy
`rok generate` and `rok sqlc …` keep their flags, their output and
their exit codes — the `sqlc` tokens are still forwarded verbatim to
the engine — and the root help now marks them as superseded by the
front door; their removal is deferred to 1.0. One spelling changed:
`rok generate` no longer takes the `generate` subcommand, so the write
mode is flat (`rok generate [OPTIONS]`) and the CI mode nests
(`rok generate check`).

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
crates.io in dependency order (workspace ADR 0008). The repository's
`install.sh` and `install.ps1` fetch and verify those archives, so a
release installs with one line.

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
  `rok`, workspace ADR 0002).
- `rok-fluent` and `rok-orm` as separate runtimes (superseded; the
  absorbed pieces are documented in workspace ADR 0001).
