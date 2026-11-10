---
title: Migrating to `rok-data`
description: Per-repository migration notes (rok-db, rok-sqlc, rok-fluent, rok-orm) and the 0.9.0 / 0.8.0 / 0.7.0 / 0.6.0 / 0.5.0 / 0.4.0 / 0.3.0 release upgrades.
updated: 2026-11-06
kind: guide
source: crates/rok-data/tests/docs/migration_guide.rs (compiled samples), CHANGELOG.md
---

# Migrating to `rok-data`

## Agent brief

- **What this page does:** per-repository notes — what changes when a
  project moves from rok-db, rok-sqlc, rok-fluent or rok-orm into this
  workspace — plus the release upgrades **To 0.9.0**, **To 0.8.0**,
  **To 0.7.0**, **To 0.6.0**, **To 0.5.0**, **To 0.4.0** and **To 0.3.0**.
- **Upgrade recipe (any release):** regenerate once (`rok gen`, or
  `rok sqlc generate` for engine projects), then fix what the compiler
  reports. The compiler finds the breakages; this page explains each.
- **Everything not listed is unchanged** — same APIs, same tests, same
  behavior (verified by porting the original test suites).
- **Rust samples are compiled** and marked `<!-- source: … -->`; the
  `<!-- not compiled: … -->` blocks are the "before" side of comparisons.
  Edit sources under `crates/rok-data/tests/docs/`, not this page.

Per-repository note: what your project needs to change to move from one
of the four source repositories to the consolidated workspace, based on
what was actually renamed, removed, or unified. Everything not listed is
unchanged — same APIs, same test suites, same behavior (verified by
porting the original tests).

Seven sections are not about a repository lineage: **To 0.9.0**,
**To 0.8.0**, **To 0.7.0**, **To 0.6.0**, **To 0.5.0**, **To 0.4.0** and
**To 0.3.0**, the upgrades between workspace releases. The generated
output changes in 0.9.0, 0.4.0 and 0.3.0, and the runtime's public
surface in the last two; 0.5.0 changes the templates only.

## To the next release (unreleased)

### Request bodies are plainer outside a `PATCH`

A body field that may be left out and may be `null` keeps its three
states only in a `PATCH` body; in a `POST` (or `PUT`) body it is plain
optional. After `rok gen`, the compiler points at each call to change:

```text
Rust:  NewThing { note: Some(Some(text)), .. }   →  NewThing { note: Some(text), .. }
Dart:  NewThing(note: Nullable(text))             →  NewThing(note: text)
```

A `PATCH` body is unchanged: `Some(None)` / `Nullable(null)` still
clears a field.

### The TypeScript Client is built on zod 4

`rok gen` now writes each type of the TypeScript Client as a zod schema
with its type inferred from it, so the types keep their names and
imports compile unchanged. Add zod to the web App, then regenerate:

```console
$ bun add zod@^4
$ rok gen
```

- **Schemas to check input with:** `<Type>Schema` carries the server's
  rules (lengths, bounds, patterns, formats), so a form can check a body
  before it is sent: `NewUserDtoSchema.safeParse(values)`.
- **Responses are checked:** a call whose JSON success is not what the
  contract describes rejects with `ContractError` (`operation`, and
  `issues` with each `path` and `message`), beside `ApiError` for a
  failed request. Unknown keys are dropped, not rejected. Code that
  catches errors from a call can tell the two apart:

```ts
import { ApiError, ContractError } from "./lib/api"

try {
  await api.users.me()
} catch (error) {
  if (error instanceof ContractError) report(error.operation, error.issues)
  else if (error instanceof ApiError) show(error.message)
  else throw error
}
```

## To 0.9.0

Move the `rok-data` pin to `"0.9"` and regenerate. The runtime's API is
as in 0.8.1 (it gains `Raw::fetch_page`, and `&Db` and `&mut Tx`
implement `sqlx::Acquire`); what changes is what `rok gen` accepts and
generates, how the commands find the project, and what they print. Most
projects meet the first three subsections; the last two touch code that
drives the engine as a library or reads the commands' output.

### `rok gen` fails on what it can't generate

A table or query the engine can't generate fails `rok gen` and
`rok gen check`, where it was skipped with a warning. The error names the
query's file and line, the expression and the override that types it:

```sql
-- name: NameMatches :many
-- column: score f32
SELECT id, similarity(name, $1) AS score FROM products;
```

Then:

- **Type it** with the `-- column:` override the error names, as above;
- **leave it out on purpose** with `-- rok:skip [note]` after its
  `-- name:` line: the run prints a `note:` and `--json` lists it under
  `skipped_on_purpose`;
- **or pass `--lenient`** to skip every such query as before.

`--strict` is now the default and is still accepted, so it can stay in a
script. It no longer fails on the other warnings (a scaffolded file using
what the settings stopped generating). Fewer queries reach the error than
before: comparisons, arithmetic, `CASE`, `||`, `EXISTS`, subqueries,
`UPDATE … FROM`, `DELETE … USING`, `INSERT … SELECT` and calls to the
schema dir's own `CREATE FUNCTION`s are typed now (the engine's README,
"Writing query files", has the table).

### Generated query types change

Queries that were typed before can get a closer type, so a field or a
return value changes and the compiler points at each caller:

- **Casts keep nullability.** A cast was always `Option`; it now has the
  nullability of what it wraps. `count(*)::bigint AS owners` is `i64`, a
  cast of a `NOT NULL` column is its target type (still `Option` on the
  outer side of a join), and any other cast stays `Option`.
- **Literals are never NULL**: `0::int AS depth` is `i32` and
  `'active'::product_status` is `types::ProductStatus`.
- **`coalesce` is typed from all its arguments**: it takes the type they
  share (a literal takes the others') and is not `Option` when one
  argument never is. `coalesce(x, 0)` is `i64` for a `bigint` `x`, and
  `coalesce(note, sku)` is `String`, where it failed to infer.
- **A cast to a typed id's plain type keeps the typed id**:
  `media_id::uuid` over a key column is `MediaId`, not `uuid::Uuid`.

A caller drops what it did to get the old type: the `unwrap_or(0)` (or
`unwrap_or_default()`) on a field that is no longer `Option`, and the
`MediaId::from(…)` around a `Uuid` that is now a `MediaId` (a caller that
wants the `Uuid` takes `.0`).

<!-- not compiled: before and after of a caller of a generated query -->
```rust,ignore
// 0.8: `owners` was `Option<i64>`, `cover` was `Option<uuid::Uuid>`
let owners = row.owners.unwrap_or(0);
let cover = row.cover.map(MediaId::from);
// 0.9: `owners` is `i64`, `cover` is `Option<MediaId>`
let owners = row.owners;
let cover = row.cover;
```

Two parameter changes:

- **`BETWEEN` bounds** named by their column are `<column>_from` and
  `<column>_to` (`starts_on BETWEEN $2 AND $3` takes `starts_on_from`,
  `starts_on_to`), where they were `<column>` and `<column>_2`.
- **`-- param:` without a type** renames and keeps the inferred type
  (`-- param: $2 from`); `-- param: $2 reviewer!` also drops the `Option`
  a nullable column gives it. A `!` beside a type is refused: write the
  type itself.

`SCHEMA_SQL` (`db/schema.rs`) now lists the schema's statements in an
order that loads into an empty database; a schema already in such an
order keeps its text.

### Relationships of a foreign key without `_id`

A foreign key column with neither an `_id` nor an `_<referenced column>`
suffix names its relationship after the column and the referenced row:
`offers.reviewed_by` referencing `users` gives `reviewed_by_user`
(`load_reviewed_by_user`, the filter's `reviewed_by_user` field, and
`offers_by_reviewed_by_user` on `users`), where it was `reviewed_by` and
failed generation. Columns ending in `_id` keep their names. Code that
called the old names takes the new ones.

A `[relationships]` entry written for the old name
(`reviewed_by = "reviewer"`) now names no relationship and fails the run;
the error lists the new name. Rename the key:

```toml
[relationships.offers]
reviewed_by_user = "reviewer"
```

### `rok migrate` and `rok schema diff` find the project

`rok gen`, `rok gen check`, `rok migrate *`, `rok schema diff` and
`rok sqlc check-arch` walk up to the nearest `rok.toml` and act on the
App started inside, or on the root's App(s). `--dir` has no default any
more: without it, `rok migrate *` and `rok schema diff` read the
settings' `migrations` directory (`migrations` beside the settings file
when unset), not `./migrations` of the current directory. So:

- a script that ran `rok migrate up` in a directory whose `migrations/`
  differs from the project's passes `--dir` (an explicit `--dir` still
  wins);
- at a Monorepo root naming several Apps, the command asks for
  `--app <name>`;
- `check-arch` finds a `rok.toml` project, where it read only
  `rok-data-sqlc.toml`, and `--config` takes a `rok.toml`.

Each command prints the settings file and directories it read on stderr
(`using App api: apps/api/rok-data-sqlc.toml (migrations
apps/api/migrations)`). The database commands (`rok migrate
status|up|down`, `rok schema *`, `rok db *`) also load the App's `.env`;
a variable already in the environment wins, and `--no-env-file` reads
nothing. `rok db drop` guards the database the `.env` names too.

### Migrations and line endings

- **Line endings don't change a staged migration.** `rok gen --migration`
  reads the truth files and the `.rok-schema.json` snapshot with CRLF
  turned into LF, so a CRLF checkout of an unchanged schema stages
  nothing. A snapshot recorded from a CRLF file needs no rewrite.
- **The checksum ignores line endings.** A migration's checksum
  (`Migration::checksum`, the journal's `checksum` column) is computed
  over its `up` SQL with CRLF turned into LF: an LF file keeps the
  checksum it had, and its CRLF copy now has the same one. A journal row
  recorded from a CRLF file before 0.9.0 holds the old checksum, of the
  CRLF bytes. It still matches, whether the file is checked out with CRLF
  or LF now, and the next `rok migrate up` records the new checksum in
  its place, once.

  The one row that does not match is a file recorded with *mixed* line
  endings and checked out differently since: `rok migrate status` calls
  it `edited`. To re-record it, run `rok migrate up` once from a checkout
  whose file has the bytes that were applied (the commit it was applied
  from); the row then holds the new checksum, which every checkout
  matches.

### What the commands print

A script or test that matches the commands' output takes the new text:

- **Errors show their whole chain**: `error: failed to parse SQL schema:
  schema/0001_init.sql:2: invalid PostgreSQL SQL syntax: <the parser's
  message>`, where it stopped at `failed to parse SQL schema`; the same in
  `--json` and in the database errors of `rok migrate *`, `rok db *` and
  `rok schema *`.
- **`rok sqlc check-arch` prints a pass line on stdout**: `checked 3
  features: no violations` (`, 2 warnings` when `--lenient` let layout
  violations through), where a clean run printed nothing. Exit codes are
  unchanged.
- **`rok gen` announces newly served tables** on stderr, once:
  ``warning: now serving `/api/v1/posts` (list, create, get_one, update,
  remove); add `posts` to `[api] exclude` to keep it private``; `--json`
  carries them in `newly_served`. It also prints `exporting the contract
  …` before the contract export, and a failed run of several steps ends
  with the steps done, the one that failed, the ones skipped and the
  command that finishes the rest (`--json`: `{"steps": [...], "finish":
  ...}`). A Monorepo's `rok gen --json` no longer prints the `api:`
  heading on stdout.
- **Feature folders no longer generated** (the table excluded or
  dropped) lose their scaffolded `service.rs` while it is unedited; an
  edited one stays and is listed (`--json`: `unwired`).
- **`check-arch` rule 4's message** ends "; a pure helper with no feature
  state belongs in shared/".

To serve only some tables, `[api] include` is the allowlist alternative
to `exclude` (setting both fails the run, naming both); without either,
every table is served, as before.

### Clients: `If-Match` on a versioned `PATCH`

The Clients are now emitted for an App with a versioned table, and the
`PATCH` of such a table takes the `If-Match` header as a required
argument: `api.documents.update(id, body, { ifMatch: version })` in
TypeScript, `update(id, body, ifMatch: version)` in Dart,
`client.documents().update(&id, &body, &version)` in Rust. A bare version
is quoted as an entity tag. Code that called a hand-written or older
Client's `update` passes the row's version.

### Driving the engine as a library

Code that calls `rok_data_sqlc` directly, rather than through `rok`:

- **`queries::analyze_queries`** takes the schema's functions after the
  enums: `analyze_queries(&defs, &tables, &enums, &schema.functions,
  &generated, &unsupported)`.
- **`Schema::functions`** (`Vec<SqlFunction>`) is a new field: a
  `Schema { tables, enums }` literal adds `functions: Vec::new()`.
- **`ParamOverride::rust_type` is an `Option<String>`** (`None` keeps the
  inferred type), and `ParamOverride::not_null` is new; **`QueryDef::skip`**
  (`Option<String>`) is new. Literals of either add the field.
- **The Clients' `openapi::Operation`** gains `header_params` and
  `optional_sign_in`, and `openapi::Error` gains `UndefinedSchema { at,
  name }`, which a `match` on it handles.
- **`Report`** gains `skipped_queries` (`SkippedQuery`), `newly_served`
  (`ServedFeature`) and `unwired`; `report::QueryFailure` gains `path`;
  `ApiConfig` gains `include`. These types are `#[non_exhaustive]`, so
  only code that reads the new fields changes.
- **`Project`, `Target`, `App` and `using_line`** are the rule the
  commands use to find the project; `ApiFramework` is as in 0.8.0.

### New, and opt-in

Nothing to change for these:

- **`:page` queries**: `-- name: PartnerBookings :page` over a `SELECT`
  with its `WHERE` and `ORDER BY` generates `partner_bookings(&pool,
  params, page, per_page)` returning `rok_data::Page<PartnerBookingsRow>`.
  `Raw::fetch_page(conn, page, per_page)` pages a raw statement.
- **Shared reads**: `queries/shared/<name>.sql` generates
  `db::queries::shared::<name>`, which any feature may call; it holds
  reads only.
- **`-- rok:skip [note]`** leaves a query ungenerated on purpose (above).
- **`rok init my-api --template grpc`** writes the `grpc-api` project
  from rok-templates 0.7.0.

## To 0.8.0

Nothing to change in a project: the runtime, and what the actix and axum
frameworks generate, are as in 0.7.0. Move the `rok-data` pin to `"0.8"`
and regenerate; only the dependency hint in `db/mod.rs` changes.

What is new is opt-in, or touches code that drives the engine as a library:

- **A third framework**, `[api] framework = "grpc"` (engine ADR 0028): one
  `proto/<resource>.proto` per Resource and a tonic service per feature
  (`grpc.rs`, in place of `routes.rs` and `handlers.rs`). The engine's
  [configuring guide](../crates/rok-data-sqlc/docs/guide/configuring.md)
  says what it generates and what it refuses (a cursor-paged list,
  `openapi = true`).
- **`ApiFramework::extractor()` returns an `Option<Extractor>`**: `None`
  for the new `ApiFramework::Grpc`, whose services need no extractor. A
  caller that used the `Extractor` takes it out of the `Some`.
  `ApiFramework` is `#[non_exhaustive]`, so a `match` on it outside the
  engine already has a `_` arm; give `Grpc` its own arm where the
  wildcard's answer is wrong for it.
- **The unknown-framework error lists `grpc`**: "expected one of
  `actix_web`, `axum`, `grpc`". A test that matches its text takes the
  new list.
- **`rok server update <NAME>`** changes a registered server's settings,
  its password, or both, in place of removing and adding it again; every
  command's `--help` ends with examples.
- **A server URL without a database path** (`postgres://user@host`)
  registers no default database, as the CLI reference says, where 0.7.0
  registered an empty one.

## To 0.7.0

Nothing to change in a project: the runtime, the generated code and the
commands are as in 0.6.1. Move the `rok-data` pin to `"0.7"` and
regenerate; only the dependency hint in `db/mod.rs` changes.

Around the project (workspace ADR 0032):

- **Two more binaries**: `cargo install rok-data-cli` installs `rok-data`
  (every command), `rok-db` (`server`, `db`, `migrate`, `schema`) and
  `rok-gen` (`init`, `gen`, `generate`, `sqlc`). `rok` (rok-v2) is
  unchanged and still answers every command.
- **No binaries or installers here any more.** This repository's releases
  publish the crates only; `rok` comes from
  [rok-v2](https://github.com/ateeq1999/rok-v2) (`cargo install rok-cli`,
  or its `install.sh` / `install.ps1`). A script that downloaded a
  `rok-data-<version>-<target>` archive from these releases takes rok-v2's
  `rok` archive instead, or runs `cargo install rok-data-cli`.

## To 0.6.0

Nothing to change in a project: the runtime, the generated code and the
`rok` grammar are as in 0.5.0. Move the `rok-data` pin to `"0.6"` and
regenerate; only the dependency hint in `db/mod.rs` changes.

What changes is around the project (workspace ADR 0032):

- **The CLI's binary is `rok-data`.** `cargo install rok-data-cli` (and
  the release archives) install `rok-data`, not `rok`; every command and
  flag is the same (`rok-data migrate up`). The `rok` command comes from
  [rok-v2](https://github.com/ateeq1999/rok-v2), which links this CLI,
  so once it is installed `rok migrate up` works as before. A script that
  calls `rok` from a 0.5 install either installs rok-v2 or calls
  `rok-data`.
- **The templates have their own repository**,
  [rok-templates](https://github.com/ateeq1999/rok-templates), released
  on its own (0.6.0). `init --template` copies the same files; a
  project made from a template is unaffected.

## To 0.5.0

Nothing to change: the runtime, the generated code and `rok sqlc`'s
grammar are as in 0.4.0. Move the `rok-data` pin to `"0.5"` and
regenerate (`rok sqlc generate`). Only the dependency hint in
`db/mod.rs` changes. `rok sqlc check-arch` now accepts `src/app/`, the
wiring split into files, as it accepts `app.rs`.

### Projects made from a template: the production capabilities

The 0.5.0 templates add jobs and schedules, logs and traces, metrics, the
security baseline, draining on `SIGTERM`, a Dockerfile, `compose.yaml`,
CI and `deploy/terraform/`. They live in scaffolded files, which are
yours, so a project made from an earlier template doesn't get them by
regenerating. To take one, make a fresh project with `rok sqlc init
--template actix` (or `axum`) and copy it in. Each page of the
[templates' guide](https://github.com/ateeq1999/rok-templates/blob/main/docs/guide/index.md) names its
files:

- **Jobs** (`src/shared/jobs/`, the `jobs` feature, `app/run.rs`'s
  `worker` and `schedule`): apply migrations `0007_jobs` and
  `0008_job_schedules`, then move mail to
  the queue (`src/shared/mail/outbox.rs`).
- **Logs, traces and metrics**: `src/shared/telemetry.rs`,
  `src/shared/metrics.rs`, and `main.rs`'s start-up.
- **Security and draining**: `src/app/middleware.rs`,
  `src/shared/security/headers.rs` and `src/shared/shutdown.rs`.
- **Deploying**: `Dockerfile`, `compose.yaml`, `deploy/`,
  `.github/workflows/ci.yml` and `deny.toml`. They don't depend on the
  code above, only on the `/health/ready` route and `APP_ROLE`.

## To 0.4.0

0.4.0 is a breaking release, smaller than 0.3.0. Regenerate once (`rok sqlc
generate`), then fix what the compiler reports. Three things change.

### `find` takes the model's own key

`Model` has a `type Key`, which `#[derive(Model)]` sets: the key field's
type, or a tuple of the key fields' types in field order for a composite
key. `find`, `find_or_fail`, `find_many`, the `ModelCollection` lookups and
`audit::history` take what converts into it:

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
let product = Product::find(&db, id).await?;
// `Product::find(&db, category_id)` doesn't compile: a `CategoryId`
// is not a `Product`'s key.
let membership = Membership::find_or_fail(&db, (1, 2)).await?;
```

A composite key of the wrong length used to fail at run time
(`Error::InvalidQuery`); now it doesn't compile. A model that implements
`Model` by hand, without the derive, adds one line:

<!-- not compiled: a line added to a hand-written impl -->
```rust
impl Model for Note {
    const TABLE: &'static str = "notes";
    const PRIMARY_KEY: &'static str = "id";
    type Key = i64; // new in 0.4.0
    // …
}
```

### Generated reads are the runtime's verbs

Code generated by `rok sqlc` calls `rok_data` and fails with
`rok_data::Error` instead of `sqlx::Error`. A `match` on `sqlx::Error`
becomes a match on `rok_data::Error` (`is_not_found()`,
`is_unique_violation()`). The generated calls that are gone:

| Before (0.3)                         | Now (0.4)                                                   |
| ------------------------------------ | ----------------------------------------------------------- |
| `users::get_by_id(&db, id)`          | `User::find(&db, id)`                                       |
| `users::get_by_email(&db, email)`    | `User::by_email(email).first(&db)`                          |
| `users::exists_by_id(&db, id)`       | `User::by_primary_key(id).exists(&db)`                      |
| `users::list` / `list_page` / `count` | `User::all(&db)` / `User::query()….paginate(..)` / `User::count(&db)` |
| `search(filter, sort, page)`         | `User::search(filter)?.order_by(sort).paginate(..)`         |
| `search_after(filter, after, limit)` | `User::search(filter)?.cursor_paginate(..)`                 |
| `<table>::write::create(…)` and the other wrappers | the model's verbs (`new_user.insert(&db)`, …)  |

On a tenant table, `by_primary_key` takes the tenant first:
`Row::by_primary_key(tenant, id).first(&db)`. The engine's CHANGELOG
lists every changed generated item.

### Projects made from a template: direct uploads

The templates gain direct uploads to presigned URLs (engine ADR 0024). A
project made from an earlier template gets the generated half when it
regenerates. The scaffolded half (`src/shared/storage/`, the `media`
feature's handlers, routes and service, `src/config.rs`, `src/app.rs`) is
yours, so copy it in from a fresh `rok sqlc init --template actix` (or
`axum`) if you want the feature. Then apply migration `0006_media_uploads`,
and set the new settings if the defaults don't suit:
`MEDIA_UPLOAD_URL_TTL_SECS`, `MEDIA_DOWNLOAD_URL_TTL_SECS`,
`MEDIA_UPLOAD_SWEEP_SECS` and `STORAGE_PUBLIC_ENDPOINT`.

## To 0.3.0

0.3.0 is a breaking release: the runtime's public surface changes, and the
generated output changes with it. Regenerate once (`rok gen`, or
`rok sqlc generate` for the engine's own projects) and fix the compiler
errors the compiler finds for you. Nothing else changes — the query
builders, the executors, the hooks' behaviour and the CLI's commands,
flags, outputs and exit codes are the same.

### Renamed calls

| Before (0.2.0)        | After (0.3.0)             | What it is |
| --------------------- | ------------------------- | ---------- |
| `Model::create()`     | `Model::insert_builder()` | the insert builder: one column at a time, raw expressions, conflict handling |
| `row.save(&db)`       | `row.update(&db)`         | the row-level update by primary key; returns the stored row |
| `row.save_only(&db, Some(vec![User::NAME]))` | `row.update_only(&db, [User::NAME])` | the row-level update of the listed columns only; any list or iterator of columns, and an empty one is an error |
| `users::write::create(exec, input)` | `User::create(exec, input)` | the generated insert, now on the model |
| `users::write::update(exec, id, input)` | `User::patch(exec, id, input)` | the generated update: returns the patched row (`Result<User>`) |
| `users::write::delete(exec, id)` | `User::destroy(exec, id)` | the generated delete: `Result<()>` |
| `users::write::restore` / `delete_permanently` | `User::restore` / `User::delete_permanently` | the soft-delete pair, now on the model: `Result<()>` |
| `user_roles::write::attach` / `detach` / `sync` | `UserRole::attach` / `detach` / `sync` | a join table's pair writes, on the join model |

The module functions (`users::write::create`, …) still compile for one
release: each forwards to the model verb and is `#[deprecated]`, so the
compiler points every call site at its replacement.

**One "not found" convention.** A verb that targets one row by key fails
with `rok_data::Error::NotFound` when no row has it — the generated
`patch`, `destroy`, `restore` and `delete_permanently` exactly like the
model's own `update`, `delete` and `find_or_fail`. Optional lookups stay
optional (`find` and the generated `get_by_*` return `Option`). A versioned
`patch` keeps its `UpdateOutcome`, whose `VersionConflict` is not an error.

<!-- not compiled: 0.2 code, shown for comparison -->
```rust,ignore
// before (0.2)
let user = match users::write::update(&db, id, patch).await? {
    Some(user) => user,
    None => return not_found(),
};
if users::write::delete(&db, id).await? == 0 {
    return not_found();
}
```

<!-- source: crates/rok-data-sqlc/tests/fixture-app/src/one_import.rs -->
```rust
// after
let user = match User::patch(&db, id, patch).await {
    Ok(user) => user,
    Err(error) if error.is_not_found() => return not_found(),
    Err(error) => return Err(error.into()),
};
User::destroy(&db, id).await?; // `?` carries NotFound to your error type
```

<!-- not compiled: 0.2 code, shown for comparison -->
```rust,ignore
// before (0.2)
let user = User::create()
    .set(User::EMAIL, "a@b.c")
    .exec(&db)
    .await?;
user.save(&db).await?;
```

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
let user = User::insert_builder()
    .set(User::EMAIL, "a@b.c")
    .exec(&db)
    .await?;
user.update(&db).await?;
```

`save` is **removed, not aliased**: it is the one verb whose
meaning is ambiguous (insert *or* update), and a non-optional typed key
leaves the runtime nothing to decide on. Use the explicit verbs — `insert`,
`update`, `upsert`, `upsert_on` — instead.

The generated data layer changes with the runtime: its write verbs move
onto the model and take the model's verb names (the table above):
`update` becomes `User::patch` (the instance verb `update` already means
"write this row"), `delete` becomes `User::destroy`, and a missing row is
`Error::NotFound` rather than a `None` or a `0` (*One "not found"
convention*, above): an `assert!(write::delete(...).await.unwrap())`
becomes `User::destroy(...).await.unwrap()`. The instance `update`,
`insert_builder()`, the query builders, `restore` and `delete_permanently`
keep their names. Regenerate and let the compiler find the call sites.

The **API layer keeps its 0.2 names**: the generated handlers and service
methods (`update`, `remove`, `remove_permanently`, `Remove{Row}Query`),
the OpenAPI `operationId`s and the `[api.skip]` names are unchanged, so a
`service.rs` override and an OpenAPI client keyed on `operationId` keep
working.

### New verbs

| Verb                | Signature                                  | Notes |
| ------------------- | ------------------------------------------ | ----- |
| `Model::exists`     | `fn exists(executor) -> Result<bool>`      | does the table have a row? the model's own scope applies (soft deletes, default scope, tenant) |
| `Model::where_`    | `fn where_(expr) -> Select<Self>`         | another name for `filter` |
| `Select::where_`   | `fn where_(expr) -> Self`                 | another name for `filter`, on the chain |
| `Select::get`       | `fn get(executor) -> Result<ModelCollection<Self>>` | the rows as a `ModelCollection`; `all` keeps the `Vec` |
| `Select::chunk`     | `fn chunk(session, size, f) -> Result<()>` | the rows in windows of `size`, one query each, so a large table need not fit in memory |

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
// `where` is a Rust keyword, so `where_` is the spelling; `filter`
// stays the canonical verb and the generated code keeps using it.
let admins = User::where_(User::ROLE.eq("admin")).all(&db).await?;

// one row or many, without fetching them
if User::exists(&db).await? {
    // …
}

// the same rows as `all`, as a `ModelCollection`
let users = User::query().get(&db).await?;

// the rows in windows of 100 — `db` as a reference the `async move`
// blocks copy
let db = &db;
User::query()
    .chunk(db, 100, |rows| async move {
        for user in rows {
            archive(db, user).await?;
        }
        Ok(())
    })
    .await?;
```

`chunk` reads one window per query (`LIMIT`/`OFFSET`, ordered by the
primary key when the query has no order), so only one window is in memory
and no connection is held while the callback runs — a callback that writes
back works even with a pool of one. The callback returns `Ok(())` to go on
or `Ok(false)` to stop; the first error stops the
walk and is returned.

The other table walks, for large tables:

| Call | What it does |
| ---- | ------------ |
| `.chunk_by_id(db, 100, f)` | keyset windows by primary key: correct when `f` deletes or updates the rows it was handed |
| `.lazy(db, n)?` / `.lazy_by_id(db, n)?` / `.lazy_by_id_desc(db, n)?` | a `Stream` of rows, read `n` at a time |
| `.each(db, f)` / `.each_by_id(db, f)` | one row per call, a thousand per query |

### Relations

**New in 0.3.0.** The schema's foreign keys become relation attributes on
the model: the holding field carries `#[rok(belongs_to = Target)]`, and the
referenced table gets `#[rok(has_many(children = Child::FK))]` — or
`has_one` when the foreign-key column alone is a unique key there. The
derive expands them into typed relation constants and accessors, so a call
site reads them off the row:

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
// a post belongs to its author; a user has many posts
post.author().all(&db).await?;
user.posts().where_(Post::DRAFT.eq(false)).all(&db).await?;
```

The relation name follows the column: `author_id` names the relation
`author`, and a foreign key the schema does not name unambiguously states
it — `#[rok(belongs_to(author = User))]`. Soft-deleted related rows stay
out of an accessor's query, like everywhere else a read happens.

### Named scopes

**New in 0.3.0.** A model can declare **named scopes**: reusable
constraints on a query chain, named by path. The derive gives the model an
entry (`User::popular()`) and the chain a method
(`User::popular().active()`), so scopes compose:

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
#[derive(Model)]
#[rok(scopes(popular = popular, active = active))]
struct User {
    id: i64,
    views: i32,
}

// a scope is a function from a query chain to a query chain
fn popular(query: Select<User>) -> Select<User> {
    query.filter(User::VIEWS.gt(100))
}

fn active(query: Select<User>) -> Select<User> {
    query.filter(User::VIEWS.gt(0))
}
```

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
// the model's entry
let popular = User::popular();
// the chain's method — the `UserScopes` trait the derive writes next to
// the model; generated code re-exports it from `crate::db::prelude`
let popular_and_active = User::popular().active();
```

Generated code writes a `prelude` module in `db/mod.rs`:
`use crate::db::prelude::*;` brings rok-data's prelude, every model, and
each model's `{Model}Scopes` trait. For hand-written models, re-export the
scope trait next to the model the same way (`pub use models::{User,
UserScopes};`) so callers need a single glob import.

A scope can take parameters — declared in the attribute, after the name,
in the same shape the entry takes them. The free function takes the query,
then the declared parameters; the entry and the chain method get the same
signature:

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
#[derive(Model)]
#[rok(scopes(older_than(n: i32) = older_than))]
struct User {
    id: i64,
    age: i32,
}

// the free function takes the query, then the declared parameters
fn older_than(query: Select<User>, n: i32) -> Select<User> {
    query.filter(User::AGE.gt(n))
}
```

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
// the entry and the chain method take the same shape
let adults = User::older_than(18);
let seniors = User::query().older_than(65);
```

A generated model declares its scopes through the engine's `[scopes]`
configuration instead (`[scopes.users] famous = "crate::scopes::users::famous"`):
the engine wires `#[rok(scopes(famous = …))]` onto the row, and your crate
owns the functions the paths name.

Nothing changes for the scopes that already exist: the model's
`default_scope` (`#[rok(default_scope = …)]`) and `.unscoped()` keep their
meaning, and the closure-based `Select::scope` still takes a function.

### Write verbs moved to `ModelWrite`

The `Model` trait's write methods (`insert`, `insert_all`, `copy_in`, `upsert`,
`upsert_on`, `update`, `update_only`, `delete`, `force_delete`, `restore`)
are now in a separate `ModelWrite` trait. `Model` stays the read/query
surface; `ModelWrite` is the write surface. Both are in the prelude so
`use rok_data::prelude::*;` still brings both into scope. Generated models
implement both traits automatically. Hand-written models that implement
`Model` can also implement `ModelWrite` (defaults are provided).

The `Tracked` wrapper now requires `ModelWrite` (it uses `update_only` to
save only changed columns). Its `new` constructor and the model's `track()`
method now require the model to implement `ModelWrite` — which all derived
models do.

`row.rs` is now well under the 600-line gate.

### The conventions are inferred now

A model's own field names are the signal (workspace ADR 0015):

| The model has | It now gets | The opt-out |
| --- | --- | --- |
| `created_at` and `updated_at` fields | ORM-managed timestamps: `now()` on insert, `now()` on every update | `#[rok(no_timestamps)]` |
| a `deleted_at` field | soft deletes: the reads skip the rows where it is set, and `delete` marks the row instead of removing it | `#[rok(no_soft_delete)]` |

**This changes existing models.** A model with a `deleted_at` field that was
*not* a soft-delete column — an audit trail, a column that means something
else — now soft-deletes: its reads skip the rows where the column is set,
and its `delete` marks the row instead of removing it. Add
`#[rok(no_soft_delete)]` to such a model. The same goes for a model with
both `created_at` and `updated_at` fields that the application wrote
itself: it now gets `now()` written on insert and on every update. Add
`#[rok(no_timestamps)]` to keep the application's values.

The markers `#[rok(timestamps)]` and `#[rok(soft_delete)]` keep working and
mean the same thing — they are redundant now, not wrong. Carrying a marker
*and* its opt-out (`#[rok(timestamps)]` with `#[rok(no_timestamps)]`) is a
contradiction, and the derive refuses to compile it.

The generated rows change with it: the generator stops writing the markers
and writes the opt-out only where the database owns the column. Regenerate
and read the diff — a row that now carries `#[rok(no_timestamps)]` or
`#[rok(no_soft_delete)]` is a table whose columns the database fills.

### Validation errors carry paths; constraint violations read as field errors

`FieldError::field` is an owned path now (`String`, `"address.city"`,
`"items.2.sku"` for nested input), and a `FieldError` carries the rule's
`params`. Compare with `e.field.as_str()` / `e.field == "email"`;
`ValidationErrors::add` takes any `impl Into<String>`, so calls that pass
a literal still compile.

A unique or foreign-key violation on a row write (`insert`, `insert_all`,
`upsert`, `upsert_on`, `first_or_insert`, `update`, `update_only`) comes
back as `Error::Validation` with the column as the field and code
`unique` / `exists` — the same error a database rule reports (workspace
ADR 0021) — instead of `Error::Database`. `is_unique_violation()` /
`is_foreign_key_violation()` stay true for it; `constraint()` is `None`.
Code that matched `Error::Database` for these cases matches
`Error::Validation`, or keeps calling the two predicates. A violation on
columns the model has no field for, and any violation from the builders
(`insert_builder`, `update_all`, `Insert`), stays a database error.

### Generated validation errors are the runtime's

A project made from a template carries its own `src/error.rs`. After
regenerating, change its validation import and the `rok_data::Error`
mapping:

<!-- not compiled: a line-by-line edit of a scaffolded file -->
```rust,ignore
// before
use validator::ValidationErrors;
// after
use rok_data::ValidationErrors;

// before
rok_data::Error::Validation(_) => AppError::Validation {
    message: error.to_string(),
    details: None,
},
// after
rok_data::Error::Validation(details) => AppError::from(details),
```

A client reading a `422` sees the same `details` map. The difference is
that a rule's `params` no longer echo the rejected `value`.

### Generated inputs derive `rok_data::Input`

Regenerating replaces `validator` in the generated code (engine ADR
0025). In the project:

- **The manifest:** remove `validator` (and `regex`, if only rules used
  it). Add rok-data's `regex` feature when an `[api.rules]` rule is
  `regex`, `mobile` or `postalCode`; the generated `db/mod.rs` lists it.
- **Hand-written request types** that go through `ValidatedJson` derive
  `rok_data::Input` instead of `validator::Validate`. Wrap each rule in
  `rok(…)`: `length`, `range` and `email(message = …)` keep their syntax.

<!-- not compiled: a line-by-line edit of a hand-written type -->
```rust,ignore
// before
#[derive(serde::Deserialize, validator::Validate)]
pub struct Signup {
    #[validate(email(message = "email must be an email address"))]
    pub email: String,
}
// after
#[derive(serde::Deserialize, rok_data::Input)]
pub struct Signup {
    #[rok(validate(email(message = "email must be an email address")))]
    pub email: String,
}
```

- **Calling validation yourself:** `body.validate()` becomes
  `body.validate_fields()` (field rules), or `body.validate(&db).await`
  when the type has database rules.
- **`custom` functions:** a `custom` rule takes `fn(&T) -> Result<(),
  String>`. On an `Option` field it gets the value inside, and only for
  `Some`.

### Unchanged on purpose

- `filter` stays the canonical verb (the Diesel/SeaORM convention);
  `where_` is an alias, not a replacement.
- `all` stays next to `get`.
- `Tracked::save` keeps its name: a tracked record only ever writes the
  columns that changed, so it never has the insert-or-update ambiguity
  `Model::save` had.
- The `before_save` / `after_save` hooks keep their names (and their
  meaning: they run around the row-level update).
- `Model::update_all()` (the bulk set-builder) keeps its name.

## From `rok-db`

**Rust packages:** `rok-db` → `rok-data` (facade), `rok-db-core` →
`rok-data-core`, `rok-db-macros` → `rok-data-macros`, `rok-db-codegen` →
`rok-data-codegen`. Cargo dependency lines change accordingly. The
generated model crates pin `rok_data_version`/`rok_data_path` in
`rok-data.toml`'s `[gen]` table — those keys are the former
`rok_db_version`/`rok_db_path`.

**CLI:** `rok-db-gen generate|check|migrate …` →
`rok generate|rok generate check|rok migrate …`. Flag spellings and
defaults are preserved; `rok generate` groups `check` under itself, so
old scripts calling `rok-db-gen check …` need one extra word. Usage
errors now exit 2 (clap) where the old binary exited 1 for unknown
commands; command failures still exit 1.

**Journal:** `_rok_db_migrations` → `_rok_data_migrations`. Fresh
databases get the new name automatically. Existing databases must rename
the table once (`ALTER TABLE _rok_db_migrations RENAME TO
_rok_data_migrations`): the engine hard-codes its journal, so an
unrenamed database gets a fresh (`pending`-everything) journal and `up`
would re-run already-applied SQL. The rename is the safe path;
introspection deliberately ignores both journal names either way.

**MSRV:** the workspace is `rust-version = 1.94` everywhere (it already
matched the generated-code floor for SQLx 0.9).

## From `rok-sqlc`

**Rust package:** the engine is now the library `rok-data-sqlc`. The
standalone `rok-sqlc` binary is gone; every command lives behind the
workspace CLI:

```console
$ rok sqlc generate …   # was: rok-sqlc generate  (or: rok-sqlc g)
$ rok sqlc init …       # was: rok-sqlc init     (aliases g/i/a kept)
$ rok sqlc check-arch …
```

The grammar, options, help text, stdout formats and documented exit
codes are verbatim the engine's (its test suites now drive `rok sqlc …`
end to end). **In-scope exit-code change:** none for failing commands;
usage errors keep exiting 1, as they did for the standalone binary.

**File names / environments:**

- settings: `rok-sqlc.toml` → `rok-data-sqlc.toml` (same keys inside);
- env prefix: `ROK_SQLC_*` → `ROK_DATA_SQLC_*`
  (`ROK_DATA_SQLC_TEST_DATABASE_URL`, `ROK_DATA_SQLC_UPDATE_EXPECTED` …);
- journal of its API fixtures unchanged.

**Generated files:** first-line marker changes from
`@generated by rok-sqlc …` to `@generated by rok-data-sqlc …` on the
next regeneration (one line per file; `--check` reports the drift until
then). Everything that *recognizes* generated files — the writer,
`init --template`, the fixtures — accepts both markers, and templates
shipped in this workspace keep their old-marker files, so nothing must
be regenerated immediately.

**Dependency:** the engine crate keeps versions `sqlx 0.9`, actix-web 4,
utoipa 5 as before; only the package names changed.

## From `rok-fluent`

Superseded by the rok-db lineage (workspace ADR 0001). What carries over:

- **Schema introspection** → `rok_data_core::introspect`
  (`tables`, `columns`, `indexes`, `foreign_keys`, DDL dump). The old
  `SchemaInspector` calls map:
  `list_tables()` → `introspect::tables(&db)`,
  `table_columns(&t)` → `introspect::columns(&db, t)`,
  `table_indexes(&t)` → `introspect::indexes(&db, t)`,
  `table_foreign_keys(&t)` → `introspect::foreign_keys(&db, t)`.
- **Single-file migrations** (`-- up` / `-- down` sections) are still
  read by the unified loader — `rok migrate` accepts both formats side
  by side. New files created by `rok migrate create` are pairs, and the
  versioned `<NNNN_name>` file name is now required (a breaking change
  for unversioned rok-fluent names).
- The fluent `query.filter(…).order_by(…)` API maps to
  `rok-data-core`'s `Select`/`Expr` builder; there is no drop-in shim.

## From `rok-orm`

Superseded; the entity/repository vocabulary lives on in
`rok-data-core`'s `Expr`/`Select` and the `Model` derives. See the
mapping above; no shim is provided.

## Cross-cutting compatibility notes

| Area                 | Old (4 repos)                                   | rok-data                                              |
| -------------------- | ----------------------------------------------- | ----------------------------------------------------- |
| Binary               | `rok-db-gen`, `rok-sqlc`, `rok-fluent` CLI      | `rok` (groups above)                                   |
| Runtime dependency   | sqlx 0.9 (rok-db), none (rok-sqlc)              | sqlx 0.9, only inside `rok-data-core`                  |
| Journal table        | `_rok_db_migrations`                            | `_rok_data_migrations`                                 |
| Codegen config       | `rok-db.toml` `[gen] rok_db_*`                  | `rok-data.toml` `[gen] rok_data_*`                     |
| Engine config        | `rok-sqlc.toml`                                 | `rok-data-sqlc.toml`                                   |
| Env vars             | `ROK_*`                                         | `ROK_DATA_*`                                           |
| Generated headers    | `@generated by rok-sqlc`                        | `@generated by rok-data-sqlc` (legacy headers accepted everywhere generated files are recognized) |
| Usage-error exit     | 1 (both old binaries)                           | 2 at the root; 1 inside `rok sqlc …`                   |
| MSRV                 | 1.88 (rok-sqlc) / 1.94 (generated code)         | 1.94 everywhere                                        |
| License              | MIT OR Apache-2.0                               | unchanged                                              |
