---
title: "`rok` — CLI reference"
description: Every command, flag, output and exit code of the workspace's single binary. Contract: matches crates/rok-cli/src/cli/ and the engine's grammar.
updated: 2026-10-30
kind: reference
source: crates/rok-cli/src/cli/, crates/rok-data-sqlc/src/cli.rs (engine grammar)
---

# `rok` — CLI reference

## Agent notes

- **Contract status:** this page is the user-facing summary; the exact
  grammar is `crates/rok-cli/src/cli/` — and for the `sqlc` subtree the
  engine's `crates/rok-data-sqlc/src/cli.rs`, which `rok` forwards to
  verbatim. Any grammar change must update this page in the same PR —
  matching the engine's tests — because the CLI's help text and this file
  must agree.
- **Machine readability:** `--json` (migrate/db commands, and `rok gen
  check`) means one JSON document on stdout, nothing else there;
  progress/prose on stderr. Exit codes: 0 success/help, 1 runtime
  failure (and *usage* errors inside `rok sqlc …`), 2 root usage errors.
- **Legacy subsets:** `rok generate` and `rok sqlc …` keep their flags,
  output and exit codes unchanged; removal is deferred to 1.0 (engine
  ADR 0012).

The workspace's single binary. Contract text: this file is the user-facing
summary; the exact grammar lives in `crates/rok-cli/src/cli/` and — for
the `sqlc` subtree — the engine's `crates/rok-data-sqlc/src/cli.rs`, which
`rok` forwards to verbatim.

```text
rok init [NAME]                   scaffold a project (skeleton or template)
rok gen [OPTIONS] [check]         data layer and API features from the schema dir
rok generate [OPTIONS] [check]      legacy: model-crate generator
rok migrate <COMMAND>               runs migrations on the database
rok db <COMMAND>                    a database: check, inspect, create, drop, query
rok schema <COMMAND>                live schema as DDL; journal-vs-files diff
rok server <COMMAND>                per-user server registry (ADR 0028)
rok sqlc <COMMAND>                  legacy: resource-module generator (engine)
```

Two commands are **legacy**: `rok generate` and `rok sqlc …`. Their flags,
their output and their exit codes are unchanged, and the `sqlc` tokens are
forwarded verbatim to the engine; one spelling changed: `rok generate` no
longer takes the `generate` subcommand, so the write mode is flat
(`rok generate [OPTIONS]`) and the CI mode nests (`rok generate check`).
Both are superseded by the front door (`rok gen` generates, `rok init`
scaffolds), and their removal is deferred to 1.0.

## Conventions

- `--database-url <URL>` on every database-touching command; falls back
  to the `DATABASE_URL` environment variable. URLs are always printed
  masked (userinfo dropped, query cut at `?`).
- `--json` switches the migrate/db commands to a machine contract: one
  JSON document on stdout, and *nothing else* there (progress and prose
  go to stderr). Documented shapes:
  - `migrate status --json` / `up --json` / `down --json` report
    `{"version": <i64>, "name": "...", "state": "applied|pending|edited|unknown",
    "checksum": "...", "drift": <bool>}` as `{"migrations": [StatusRow…]}`.
  - `db check --json` reports `{"ok": true, "version": "...", "pool": {…}}`;
    the `db` and `server` subsections give every other shape. A command that
    fails prints `error: …` on stderr and nothing on stdout, except
    `db query --json`, which reports the failed statement on stdout too
    (see its section).
- Exit codes: `--help`/`--version` = 0; usage errors at the root = 2
  (clap); runtime failures (bad URL, drift, check failures) = 1. Inside
  `rok sqlc …`, usage errors exit 1 — the engine's own historic behavior,
  preserved so its tests keep asserting it.
- Flags beat the settings file wherever both exist.

## `rok init` — scaffold a project

```text
rok db create  <DBNAME> [--database-url] [--server <NAME>] [--json]  create a database
rok db drop    <DBNAME> [--database-url] [--server <NAME>] [--yes] [--force] [--json]  drop a database
Usage: rok init [OPTIONS] [NAME]
```

Scaffolds a project: the application skeleton (`main`/`app`/`config`/`error`/
OpenAPI wiring, engine ADR 0016), or — with `--template` — a ready-made **Template**,
a complete starter project for one web framework. The scaffolding itself is
the engine's (`rok_data_sqlc::init`); the root command owns the grammar, the
schema seeding and the report.

| Flag                  | Meaning                                                            |
| --------------------- | ------------------------------------------------------------------ |
| `-t, --template NAME` | copy this Template instead of the skeleton (`actix`, `axum`, or `monorepo`)      |
| `-w, --api-framework FW` | the skeleton's web framework: `actix_web` (the default) or `axum`; not with `--template`, which has its own |
| `-s, --schema PATH`   | seed the truth directory (`schema/`) with this file's content       |

`NAME` is the project directory: it is created when missing, and the current
directory is the project when `NAME` is left out. Every file it writes is a
**scaffolded file**: written once, never rewritten. A second run only fills in
what is missing; a file the user has edited is kept as it is. Cargo's
untouched default `src/main.rs` (the one `cargo new` writes) is replaced, and
the run says so; any other `main.rs` is kept.

`--schema PATH` copies that file to `schema/0001_init.sql`, so the truth
directory — the project's only source of truth — starts non-empty. The
skeleton seeds `schema/` empty; the migration history (`migrations/`)
appears with the first staged pair. An existing `schema/0001_init.sql`
is kept, never overwritten, and the run says that it kept it.

The run prints what it wrote and the next steps, which name the front door:
`rok gen` turns the truth directory into `src/db/` and `src/features/`.

An unknown `--template` is a usage error (exit 2) and names the valid
templates. A failure while scaffolding (an unwritable directory, an
unreadable `--schema` file) prints `error: …` on stderr and exits 1.

## `rok gen` — the front door: the data layer from the schema dir

```text
rok db create  <DBNAME> [--database-url] [--server <NAME>] [--json]  create a database
rok db drop    <DBNAME> [--database-url] [--server <NAME>] [--yes] [--force] [--json]  drop a database
Usage: rok gen [OPTIONS] [COMMAND]
```

Two things, both over the project's schema dir: the data layer
(`src/db/**`), the shared tools and the API features (`src/features/**`)
come from the engine's generator, and `--migration NAME` stages a
migration pair with the codegen crate's diff engine. The schema dir is the
project's only source of truth (workspace ADR 0010).

| Flag                       | Meaning                                                            |
| -------------------------- | ------------------------------------------------------------------ |
| `-c, --config FILE`        | settings file (`rok.toml` `[generate]` table; the engine's `rok-data-sqlc.toml` also accepted) |
| `-s, --schema DIR`         | the truth dir (default: the config's `schema`, else `migrations/`) |
| `--migrations-dir DIR`     | the history dir the pairs stage into (default: the config's `migrations`, else `migrations/`) |
| `-o, --output DIR`         | where the modules are written (default `src/db`)                    |
| `-r, --resource TABLE`     | generate one table plus the resources already generated             |
| `-q, --queries PATH`       | annotated query file or directory; repeatable                       |
| `-w, --api-framework FW`   | the API backend: `actix_web` (the default) or `axum`                |
| `--migration NAME`         | stage a migration pair for the schema changes since the last one    |
| `--allow-destructive`      | allow the steps that lose data or fail on existing rows             |
| `--rename OLD=NEW`         | `users=accounts` (table) or `users.name=full_name` (column); repeat|
| `--strict`                 | fail on warnings (a skipped table or query)                        |
| `--json`                   | one JSON object on stdout                                           |
| `-n, --dry-run`            | plan the run; write nothing                                         |
| `-f, --force`              | overwrite files the generator did not write                          |

The settings file: `rok.toml` (the canonical name, workspace ADR 0011) holds the
generation settings in its `[generate]` section, beside `[api]` and
`[conventions]`; the engine's `rok-data-sqlc.toml`, whose keys sit at the
top level, is read as well. Flags beat the file.

### Monorepo projects: `[apps]`

A `rok.toml` whose `[apps]` section names the Apps of a monorepo (workspace
ADR 0027) changes `rok gen`'s behavior: it regenerates every named App from
its own settings file inside the App's directory, and stages each App's
migrations into its own history dir. The flags that shape one project
(`--schema`, `--output`, `--queries`, `--resource`, `--migrations-dir`,
`--api-framework`) are refused — they belong to one App, and saying them in
that App's directory does.

```toml
[apps]
api = "apps/api"
```

The `--json` output becomes an array of objects, one per App, each carrying
an `"app"` field with the App's name. Prose output prints a one-line banner
(`api:`) before each App's report. `rok gen check` aggregates: the generated
code is up to date only when every App is clean.

The `[api]` section holds the API features' settings. Its `framework` key
picks the web framework they are generated for: `actix_web` (the default)
or `axum`; `--api-framework` sets it from the command line. The framework
needs its extractor in `extractors` (`actix_web` or `axum`), and the flag
needs the `[api]` section: without it the run generates no API features
and says so. Its `pagination` key picks the JSON shape a list answers a
page in: `compact` (the default, `{items, page, per_page, total}`),
`paginator` or `resource` (with links built from the request; see the
engine's configuring guide). An unknown value fails with the valid ones.
Its `cursor` key lists the tables whose list pages by cursor instead of
by page number (`?cursor=`, answering the cursors of the pages on either
side, in the `pagination` shape); the run fails on a listed table without
a primary key or with a nullable sort key. `[api.rules.<table>]` maps a
text column to string rules and transforms by VineJS name (`"email"`,
`"minLength(3)"`, `{ regex = "..." }`; engine ADR 0019), which the create
and update bodies check and apply. A column's rules from the JSON payload
replace the file's. The run fails, writing nothing, on an unknown rule, bad
arguments, a pattern that is no regular expression, a rule given twice, a
rule on a column that isn't plain text or that generated code never
writes, and on the rules left out (`confirmed`, `sameAs`, `notSameAs`,
`activeUrl`).

The truth dir holds the **truth** (plain `*.sql` files) and the
history dir holds the **history** (the staged
`NNNN_name.up.sql` / `NNNN_name.down.sql` pairs and their
`.rok-schema.json` snapshot; workspace ADR 0004's amendment names the
split). The generator reads the truth only, so a staged migration is
never applied twice; the staged pairs use the runtime's
`NNNN_name.up.sql` convention.

`--migration NAME` diffs the truth against the state of the last staged
migration (the snapshot `.rok-schema.json` in the history dir) and writes
`NNNN_NAME.up.sql` plus — when the steps can be undone —
`NNNN_NAME.down.sql` into the history dir. An existing migration is never
overwritten, and the truth dir is never written. A step that loses data
(a dropped column or table, a new `NOT NULL` column without a default) is
refused unless `--allow-destructive` is given; `--rename` records a
rename instead of a drop and an add.

The next steps name the staged pair's file: apply it with `rok migrate
up` (with `--dir <path>` when the history dir is not `migrations`).
An `ALTER TABLE` the truth carries folds into the truth's tables when
the model can represent it; a clause it cannot rides the pair as an
extra step with the undo the truth's `-- rok:down` marker supplies.

The run prints what it wrote, what it skipped and what it refused, and the
numbered next steps. `--json` prints one object instead:
`{"written": [...], "scaffolded": [...], "unchanged": [...], "refused":
[{"path": ..., "reason": ...}], "stale": [...], "missing": [...],
"changed": [...], "skipped": [{"kind": "table"|"query", "name": ...,
"reason": ...}], "warnings": [...], "migration": {"version": ..., "name":
..., "up": ..., "down": ..., "reversible": ..., "steps": [...]},
"pending_migration": {"version": ..., "name": ..., "since": ...} | null,
"drift": <bool>, "dry_run": <bool>}`. `pending_migration` is the check's
own finding of an un-staged truth change (null otherwise): the version
the pair would take, the placeholder name `pending`, and `since` — the
last staged migration's stem. The check names no file that does not
exist: the pending pair is not among the `missing` files.

### `rok gen check [OPTIONS]`

The CI mode: nothing is written, the missing, changed and stale files are
reported, and the run exits 1 when there is drift (`--json` reports the
drift and exits 1 as well). An un-staged truth change — a truth that
moved on since the last staged pair — is a finding of its own: the prose
names the migration the truth moved on from (`0001_init`), the command
that stages it (`rok gen --migration <name>`) and the version the pair
would take (`0002_pending`); `--json` carries the same in
`pending_migration`; and the run still exits 1. The same flags
minus `--migration` and `--rename`, which belong to the generate run:
given to `rok gen` together with `check` they are an error, given to
`rok gen check` they are a usage error. The leaf's flags win over the
group's; the booleans OR.

## `rok generate` — whole-crate generation (legacy)

```text
rok db create  <DBNAME> [--database-url] [--server <NAME>] [--json]  create a database
rok db drop    <DBNAME> [--database-url] [--server <NAME>] [--yes] [--force] [--json]  drop a database
Usage: rok generate [OPTIONS] [COMMAND]
```

Legacy: superseded by `rok gen`, the front door's generator. Its flags,
its output and its exit codes are unchanged; the command no longer takes
the `generate` subcommand — the write mode keeps the original `rok-db-gen`
flat shape (`rok generate [OPTIONS]`) and the CI mode nests as
`rok generate check`. Its removal is deferred to 1.0.

| Flag                       | Meaning                                                            |
| -------------------------- | ------------------------------------------------------------------ |
| `-c, --config FILE`        | settings file (`rok.toml` `[generate]` table; legacy `rok-data.toml` `[gen]` accepted) |
| `-i, --input DIR`          | directory holding the `.sql` sources (default `db`)                |
| `-o, --output DIR`         | directory of the generated crate (default `db-gen`)                |
| `--crate-name NAME`        | package name of the generated crate (default `db-gen`)             |
| `--migration NAME`         | append `/down` migration for schema changes since the last run     |
| `--rename OLD=NEW`         | `users=accounts` (table) or `users.name=full_name` (column); repeat|
| `--allow-destructive`      | with `--migration`: permit DROP/data-losing steps                   |
| `--strict`                 | fail on generator warnings (queries missing tenant/soft-delete)     |

Discovery: `rok.toml` is the canonical name and only its `generate`
section is read here; the legacy `rok-data.toml` stays readable and
prints a note, and the legacy name loses when both files exist.

The legacy command's keys (`input`, `output`, `crate_name`,
`rok_data_version`, `rok_data_path`, `strict`) and the front door's keys
(`schema`, `output`, `queries`, `resource`, `derive`, `extractors`, …)
share the `[generate]` section, and `output` means two different things
in it. A `rok.toml` written for `rok gen` is therefore not a file for
this legacy command; the config spine (Phase 0 of
`docs/unified-pipeline-plan.md`) reconciles the two. Until then, the
legacy command says so in one line — the `[generate]` section belongs to
`rok gen`, and the legacy generator reads its own `[gen]` section in the
legacy `rok-data.toml`.

`--migration` needs a database connection (DATABASE_URL). `--strict`
requires an explicit `strict` value in the config file or no default.

### `rok generate check [OPTIONS]`

Same flag set minus `--migration`/`--rename` (rejecting them is the
original rule; passing them now errors instead of being silently
ignored). Compares generated files against a fresh generation: prints
`… is up to date` and exits 0, or lists the drifted files on stderr and
exits 1. Values given both to `rok generate` and to the nested `check`
resolve with **the value on `check` winning**; boolean flags OR.

## `rok migrate` — file-pair migrations

Journal table: `_rok_data_migrations`. Files:
`<dir>/<NNNN_name>.up.sql` + `.down.sql` pairs (nnn = `0001_init`, …).

| Command                    | Flags                                        | Effect                                                     |
| -------------------------- | -------------------------------------------- | ---------------------------------------------------------- |
| `rok migrate create NAME`  | `--dir` (default `migrations`)               | writes the next-versioned up/down pair                      |
| `rok migrate status`       | `--dir`, `--database-url`, `--json`          | every file + its journal state (`applied`, `pending`, `edited`, `unknown`) |
| `rok migrate up`           | `--dir`, `--database-url`, `--json`          | applies pending files in version order                      |
| `rok migrate down`         | `--dir`, `--database-url`, `--steps N` (default 1), `--json` | reverts the latest; `rollback` is an alias    |

State semantics: `applied` (checksum matches the journal), `pending`
(not yet applied), `edited` (file changed after being applied — the
engine refuses to skip it silently), `unknown` (journal row without a
matching file). Checksum drift on an applied file is fatal to `up` /
`down` unless the row is reported through `--json` (`drift` field).

## `rok db` — a database: connectivity, structure, create, drop, query

```text
rok db check   [--database-url <URL>] [--json]
rok db inspect [--table <TABLE>] [--database-url <URL>] [--json]
rok db create  <NAME> [--database-url <URL>] [--server <NAME>] [--json]
rok db drop    <NAME> [--database-url <URL>] [--server <NAME>] [-y|--yes] [--force] [--json]
rok db query   <SQL>  [--database-url <URL>] [--server <NAME>] [--json]
```

`check` and `inspect` act on the project's database: `--database-url`,
else `DATABASE_URL`. `create`, `drop` and `query` act on a server and
also take `--server` (workspace ADR 0028).

### Connection resolution: `create`, `drop`, `query`

1. `--database-url <URL>` — an ad-hoc connection; wins over the others.
2. `--server <NAME>` — a registered server (see `rok server`): host,
   port, user, `sslmode` from `servers.toml`, the password from the OS
   keychain or `ROK_SERVER_<NAME>_PASSWORD`. The connection goes to the
   server's `database` field (`postgres` when it has none).
3. `DATABASE_URL` — the project's database.

The flags do not conflict: given both, `--database-url` is used. With
none of the three, the command fails with
`needs a database: set DATABASE_URL or pass --database-url`. An unknown
server fails with `server '<NAME>' not found`; a server without a password
fails naming the variable to set.

### `rok db check [OPTIONS]`

Connects, pings, and reports the server version and the pool settings.

| Flag                   | Meaning                                         |
| ---------------------- | ----------------------------------------------- |
| `--database-url <URL>` | The database to check (default: `DATABASE_URL`) |
| `--json`               | Print one JSON object instead of prose          |

```text
$ rok db check
ok: connected to postgres://localhost:5432/app
pool: 1 of 1 connections
server: PostgreSQL 17.0 on x86_64-pc-linux-musl, compiled by gcc …
```

`--json`: `{"ok": true, "version": "<SELECT version()>", "pool":
{"connections": <n>, "idle": <n>, "max_connections": <n>}}`.

### `rok db inspect [OPTIONS]`

Lists the user tables of schema `public`, or with `--table` one table's
columns (name, type, primary key, nullability), indexes and foreign keys,
read through `rok-data-core::introspect` (`information_schema` /
`pg_catalog`).

| Flag                   | Meaning                                           |
| ---------------------- | ------------------------------------------------- |
| `--table <TABLE>`      | Restrict the output to one table                  |
| `--database-url <URL>` | The database to inspect (default: `DATABASE_URL`) |
| `--json`               | Print one JSON object                             |

Text output: one table name per line; with `--table`, one line per column
(`name type[ pk][ null]`). `--json`: `{"tables": ["…"]}`; with `--table`,
`{"table": "…", "columns": [{"name", "type", "nullable", "default",
"primary_key"}], "indexes": [{"name", "columns", "unique", "primary"}],
"foreign_keys": [{"name", "columns", "references": {"table", "columns"}}]}`
(an unknown table is an error under `--json`).

### `rok db create <NAME> [OPTIONS]`

Creates database `NAME` (`CREATE DATABASE`, the name quoted as an
identifier) on the target. With neither `--server` nor `--database-url`,
it creates on the server the project's `DATABASE_URL` names, and says so
first on stderr:
`warning: no --server or --database-url; creating on the server DATABASE_URL names (<masked URL>)`.

| Flag                   | Meaning                                                    |
| ---------------------- | ---------------------------------------------------------- |
| `NAME`                 | Name of the database to create                             |
| `--database-url <URL>` | The database to connect to (default: `DATABASE_URL`)       |
| `--server <NAME>`      | Use a registered server (default: `DATABASE_URL`)          |
| `--json`               | Print one JSON object                                      |

```text
$ rok db create myapp_staging --server staging
Database 'myapp_staging' created on server 'staging'.

$ rok db create test_db --database-url postgres://user:pass@host/postgres
Database 'test_db' created.
```

`--json`: `{"database": "<NAME>", "created": true}`, plus `"server":
"<NAME>"` when `--server` was given.

### `rok db drop <NAME> [OPTIONS]`

Drops database `NAME` (`DROP DATABASE`, the name quoted as an identifier)
on the target.

| Flag                   | Meaning                                                         |
| ---------------------- | --------------------------------------------------------------- |
| `NAME`                 | Name of the database to drop                                    |
| `--database-url <URL>` | The database to connect to (default: `DATABASE_URL`)            |
| `--server <NAME>`      | Use a registered server (default: `DATABASE_URL`)               |
| `-y`, `--yes`          | Skip the typed-name confirmation prompt (for scripts)           |
| `--force`              | Allow dropping the project's own database (from `DATABASE_URL`) |
| `--json`               | Print one JSON object                                           |

Two guards, checked before anything is dropped:

1. **The project's database.** When `NAME` is the database the
   environment's `DATABASE_URL` names (compared case-insensitively),
   the drop is refused unless `--force` — whichever connection
   (`--database-url`, `--server`) the drop itself uses.
2. **Typed confirmation.** Without `--yes`, the command prints
   `Type the database name to confirm: ` on stderr (stdout stays the
   result's, `--json` included) and reads one line; anything but
   the exact name fails with
   `confirmation failed: database name does not match`.

```text
$ rok db drop old_db --server local
Type the database name to confirm: old_db
Database 'old_db' dropped.

$ rok db drop myapp_prod --server prod --yes
Database 'myapp_prod' dropped.

$ rok db drop myapp --server local --yes   # myapp is the project's DATABASE_URL database
error: refusing to drop the project's own database (DATABASE_URL). Use --force to override.

$ rok db drop myapp --server local --yes --force
Database 'myapp' dropped.
```

`--json`: `{"database": "<NAME>", "dropped": true}`, plus `"server":
"<NAME>"` when `--server` was given.

### `rok db query <SQL> [OPTIONS]`

Runs `SQL` in **one transaction that commits only when every statement
succeeds** (workspace ADR 0028): like `psql -c`, with a rollback on any
error.

| Flag                   | Meaning                                              |
| ---------------------- | ---------------------------------------------------- |
| `SQL`                  | The SQL to run; several statements are separated by `;` |
| `--database-url <URL>` | The database to connect to (default: `DATABASE_URL`) |
| `--server <NAME>`      | Use a registered server (default: `DATABASE_URL`)    |
| `--json`               | Print one JSON object                                |

Transaction semantics:

- `SQL` is split into statements at each `;` outside quoted text and
  comments: `'…'` strings (with `''`), `E'…'` strings (with `\'`),
  `"…"` identifiers, dollar quotes (`$$…$$`, `$tag$…$tag$`), `--` line
  comments and nesting `/* … */` comments. Pieces holding only comments
  or whitespace are not statements; no statement at all is an error
  (`no SQL statement to run`).
- One connection; `BEGIN`; each statement in order; `COMMIT` after the
  last one. The first failing statement rolls the whole transaction back
  — DDL included — and the command exits 1 with PostgreSQL's message.
- Statements that would end the transaction early are refused before
  connecting: `BEGIN`, `START`, `COMMIT`, `END`, `ABORT`, `ROLLBACK`
  (`ROLLBACK TO SAVEPOINT` is allowed) and `PREPARE TRANSACTION`.
  Statements PostgreSQL refuses inside a transaction (`CREATE DATABASE`,
  `VACUUM`, …) fail and roll back like any other error.

Text output: each statement with a result shape (`SELECT`, `… RETURNING`,
`SHOW`, …) prints an aligned table — header, rule, rows (`NULL` as an empty
cell), `(N rows)` — and the run ends with the commit line:

```text
$ rok db query "CREATE TABLE t (id serial PRIMARY KEY); INSERT INTO t DEFAULT VALUES;" --server staging
OK: 2 statements executed, transaction committed.

$ rok db query "SELECT id, 'a' AS note FROM t" --server staging
id | note
---+-----
1  | a
(1 row)
OK: 1 statement executed, transaction committed.

$ rok db query "INSERT INTO t VALUES (2); SELECT 1/0;" --server staging
error: statement 2 failed: division by zero
Transaction rolled back.
```

`--json` on success:

```json
{"statements": 2, "committed": true, "results": [
  {"sql": "INSERT INTO t VALUES (2)", "rows_affected": 1},
  {"sql": "SELECT id, note FROM t", "columns": [{"name": "id", "type": "INT4"}, {"name": "note", "type": "TEXT"}],
   "rows": [["1", "a"], ["2", null]], "row_count": 2}
]}
```

Every value is PostgreSQL's text form of it (a string), `null` for
`NULL`; `type` is the PostgreSQL type name. A statement without a result
shape reports `rows_affected`. On failure, stdout holds
`{"statements": <n>, "committed": false, "error": {"statement": <1-based
index>, "message": "<PostgreSQL's message>"}}`, stderr the error line, and
the exit code is 1.

## `rok schema` — live schema, and journal consistency

```text
rok schema dump [--database-url]       approximate CREATE TABLE DDL for every user table
rok schema diff [--database-url] [--dir migrations]
                                       compares the journal's applied migrations with
                                       the files in --dir (missing, extra, drifted)
```

`diff` is the CI guard: exit 1 when journal and files disagree.

## `rok server` — per-user server registry (ADR 0028)

```text
rok server add    <NAME> [--url <URL>] [--host <HOST>] [--port <PORT>] [--user <USER>]
                         [--database <DB>] [--sslmode <MODE>] [--password-prompt] [--json]
rok server list   [--json]
rok server remove <NAME> [-y|--yes] [--json]
```

A **Server** is a named PostgreSQL connection a user registers once and
reaches from any project with `rok db create|drop|query --server <NAME>`.

- **Where:** `servers.toml` in the user's config directory —
  `$XDG_CONFIG_HOME/rok` (or `~/.config/rok`) on Linux,
  `~/Library/Application Support/rok` on macOS, `%APPDATA%\rok` on
  Windows. `ROK_CONFIG_DIR` overrides the directory. Each entry holds
  `name`, `host`, `port`, `user`, and optionally `database` and
  `sslmode`; never a password.
- **Passwords:** the OS keychain (macOS Keychain, Windows Credential
  Manager, Linux Secret Service) under service `rok`, account `<NAME>`.
  Reading falls back to `ROK_SERVER_<NAME>_PASSWORD` (the name upper-cased,
  `-` as `_`) when the keychain has no entry or none is reachable
  (headless Linux, CI). `ROK_KEYCHAIN=off` skips the keychain entirely,
  so the variable is the only source.

### `rok server add <NAME> [OPTIONS]`

Registers a server, from a URL or from discrete flags.

| Flag                | Meaning                                                                                       |
| ------------------- | --------------------------------------------------------------------------------------------- |
| `NAME`              | Name of the server (used as the keychain account)                                             |
| `--url <URL>`       | `postgres://user:password@host:port/db?sslmode=…` (`postgresql://` too); conflicts with `--host`, `--port`, `--user`, `--database`, `--sslmode`, `--password-prompt` |
| `--host <HOST>`     | Hostname or IP address; requires `--user`                                                     |
| `--port <PORT>`     | Port number (default: 5432)                                                                   |
| `--user <USER>`     | Database user                                                                                 |
| `--database <DB>`   | Default database for admin operations (default: `postgres`)                                  |
| `--sslmode <MODE>`  | SSL mode: `disable`, `allow`, `prefer`, `require`, `verify-ca`, `verify-full`                 |
| `--password-prompt` | Prompt for the password on standard input (the input is not hidden)                          |
| `--json`            | Print one JSON object                                                                         |

The URL needs a host and a user; a URL without a port means 5432, without
a path no default database. The password (from the URL, or from the
prompt, which `--password-prompt` prints on stderr) is stored in the
keychain. With no password given, or when storing fails — no keychain, or
`ROK_KEYCHAIN=off` — a warning on stderr names `ROK_SERVER_<NAME>_PASSWORD`
and the server is still added, its password to come from that variable. A
name already registered is an error (`server '<NAME>' already exists`).

```text
$ rok server add local --url postgres://postgres:secret@localhost:5432/postgres
Server 'local' added. Password stored in OS keychain (service: rok, account: local).

$ ROK_KEYCHAIN=off rok server add ci --url postgres://ci:secret@ci-db.example.com/postgres
warning: Couldn't access platform secure storage: the OS keychain is off (ROK_KEYCHAIN=off); set ROK_SERVER_CI_PASSWORD instead
Set ROK_SERVER_CI_PASSWORD in the environment to provide the password.
Server 'ci' added (password from ROK_SERVER_CI_PASSWORD).

$ rok server add ci --url postgres://ci@ci-db.example.com/postgres   # no password in the URL
warning: no password given; set ROK_SERVER_CI_PASSWORD instead
Set ROK_SERVER_CI_PASSWORD in the environment to provide the password.
Server 'ci' added (password from ROK_SERVER_CI_PASSWORD).
```

`--json`: `{"name": "<NAME>", "added": true,
"password_stored_in_keychain": <bool>}`.

### `rok server list [OPTIONS]`

Lists the registered servers and where each one's password comes from;
never the password itself.

| Flag     | Meaning               |
| -------- | --------------------- |
| `--json` | Print one JSON object |

```text
$ rok server list
NAME         HOST                            PORT USER         DEFAULT DB   PASSWORD
------------------------------------------------------------------------------------------
local        localhost                       5432 postgres     postgres     keychain ✓
ci           ci-db.example.com               5432 ci           postgres     env ROK_SERVER_CI_PASSWORD ✓
staging      staging-db.example.com          5432 rok          -            missing ✗
```

With none registered it prints `No servers registered.` `--json`:
`{"servers": [{"name", "host", "port", "user", "database", "sslmode",
"password": {"source": "keychain|env|none", "present": <bool>}}]}`
(`database` and `sslmode` are `null` when unset).

### `rok server remove <NAME> [OPTIONS]`

Removes the server from `servers.toml` and deletes its keychain entry.
When there is no keychain to delete from (none reachable, or
`ROK_KEYCHAIN=off`), it says to unset the variable instead and reports
`"password_deleted_from_keychain": false`.

| Flag          | Meaning                  |
| ------------- | ------------------------ |
| `NAME`        | Name of the server to remove |
| `-y`, `--yes` | Skip the confirmation prompt |
| `--json`      | Print one JSON object    |

Without `--yes` (and without `--json`) the command prints
`Type the server name to confirm: ` on stderr and needs the exact name. An
unknown name is an error (`server '<NAME>' not found`).

```text
$ rok server remove ci
Type the server name to confirm: ci
Server 'ci' removed. Password deleted from OS keychain.

$ ROK_KEYCHAIN=off rok server remove ci --yes
warning: Couldn't access platform secure storage: the OS keychain is off (ROK_KEYCHAIN=off); set ROK_SERVER_CI_PASSWORD instead
Unset ROK_SERVER_CI_PASSWORD if no longer needed.
Server 'ci' removed. Unset ROK_SERVER_CI_PASSWORD if no longer needed.
```

`--json`: `{"name": "<NAME>", "removed": true,
"password_deleted_from_keychain": <bool>}`.

## `rok sqlc` — resource-module generator (delegated engine, legacy)

Legacy: superseded by the front door — `rok gen` for the generation,
`rok init` for the scaffolding. The tokens are still forwarded verbatim to
the engine, so its grammar, its output and its exit codes are unchanged;
its removal is deferred to 1.0.

`rok sqlc …` is parsed by the engine's own clap grammar (`rok-data-sqlc`
is a library; `rok` forwards the tokens verbatim). Subcommand set:

| Command        | Alias | One-liner                                                                 |
| -------------- | ----- | ------------------------------------------------------------------------- |
| `generate`     | `g`   | parse the schema and emit `src/db/<table>/{…}` modules, optional api/openapi |
| `init`         | `i`   | scaffold a project (engine ADR 0016 skeleton, `-w axum` for axum, or `--template actix`/`axum`/`monorepo`), files-if-absent |
| `check`        | `c`   | the generated code is up to date; with `--db`, every annotated query agrees with the database; any difference → exit 1 |
| `check-arch`   | `a`   | check `src/` layout against the engine's ADR 0016 rules; violations → exit 1 |
| `query add`    | `q a` | append one annotated query to a query file and generate; kept only if it generates |

Executive summary of the main flags (full text: `rok sqlc generate --help`):

| Flag                  | Meaning                                                              |
| --------------------- | -------------------------------------------------------------------- |
| `-s, --schema PATH`   | SQL file, or a migrations directory read by filename order; repeatable |
| `-o, --output DIR`    | output directory (default `src/db`)                                   |
| `-c, --config FILE`   | config file instead of discovered `rok-data-sqlc.toml`                |
| `-j, --json JSON`     | options as JSON (inline, `@FILE`, `-` for stdin)                      |
| `-a, --api-output DIR`| API feature layer into this directory                             |
| `-w, --api-framework FW`| the API backend: `actix_web` (the default) or `axum`             |
| `-d, --derive <NAME>` | extra derives (`serde`, `validate`, …); `-e` engine features           |
| `-W, --watch`         | regenerate on every change to the schema, the query files or the config file, until interrupted (not with `-k`/`-n`) |

`init` next steps and the `--template` list (`actix`, `axum`, `monorepo`) print on stdout;
the scaffolder never overwrites existing files. Templates ship their own
`README.md` with the dependency list to add.

### `rok sqlc query add <NAME> <SQL> [OPTIONS]`

```console
$ rok sqlc query add OwnedMedia "SELECT * FROM media WHERE id = \$1 AND owner_id = \$2 LIMIT 1" --file media
added `OwnedMedia` to queries/media.sql
```

It appends `-- name: <NAME> :<mode>` and the SQL (ending in `;`) to the
query file, then generates the same way `rok sqlc generate` does with the
discovered config. If generation fails, the file is restored (a new file is
removed), and the error names the query's failure and its line.

| Flag                  | Meaning                                                              |
| --------------------- | -------------------------------------------------------------------- |
| `NAME`                | the query's name, PascalCase: `OwnedMedia` generates `owned_media`; must be new in the file |
| `SQL`                 | the statement, with `$1`, `$2`, … placeholders                       |
| `-f, --file FILE`     | a stem (`media`) under the one configured `queries` directory, or a path ending in `.sql` |
| `-o, --one`, `-m, --maybe-one`, `-M, --many`, `-x, --exec`, `-r, --execrows` | the return mode, at most one; omitted, it is inferred |
| `-c, --config FILE`   | config file instead of discovered `rok-data-sqlc.toml`                |

The inferred mode:

- `INSERT … RETURNING` is `:one`;
- any other statement returning rows (a `SELECT`, a `RETURNING`) is `:many`, or `:maybe_one` when it ends in `LIMIT 1`;
- an `INSERT`, `UPDATE` or `DELETE` without `RETURNING` is `:execrows`.

### `rok sqlc generate --watch`

```console
$ rok sqlc g --watch
generated src/db/media/models.rs
…
watching rok-data-sqlc.toml, schema, queries for changes (Ctrl-C stops)
regenerated in 84 ms: 1 written, 0 removed, 82 unchanged
```

The first run prints the whole report; each later run prints its failures
and warnings, then one line. A failed run (an unsupported query, say, with
its file and line) does not end the watch. The inputs are polled every
250 ms, and a change waits until they stop changing, so one save
regenerates once. Migrations are never applied: that stays `rok migrate up`.

### `rok sqlc check [OPTIONS]`

```console
$ rok sqlc check --db
generated code is up to date (83 files)
queries/media.sql:75: BigMedia: parameter `$1` (`min_size`) is `INT8` in the database; the generated code uses `i32`
Error: 1 mismatch(es) in 51 queries checked against the database
```

It fails when the generated code is out of date, as `generate --check`
does, with the discovered config. With `--db`, it then prepares every
annotated query against the database, without running it (engine ADR 0026),
and compares the parameter types, the result columns' names and types, and
their nullability with the generated code. Each difference is a line
`file:line: Query: message` on stdout. A type the check does not know (a
user's own `-- param:` type) is a note on stderr, not a failure.

| Flag                  | Meaning                                                              |
| --------------------- | -------------------------------------------------------------------- |
| `-d, --db [URL]`      | also check the queries against this database; bare, `DATABASE_URL`   |
| `-l, --lenient`       | skip unsupported tables and queries with warnings                    |
| `-c, --config FILE`   | config file instead of discovered `rok-data-sqlc.toml`                |

Apply the migrations first (`rok migrate up`): the queries are checked
against the database as it is.


`check-arch` prints each violation as `path:line: rule: message`. The rules
are the project's `AGENTS.md` six (the architecture guide states them):

| Rule                  | Reports                                                                   |
| --------------------- | ------------------------------------------------------------------------- |
| `layout`              | a feature directory holding other files than the fixed set (`--lenient`: warnings) |
| `rule-1`              | the `[api] framework`'s crate outside a feature's `routes.rs`/`handlers.rs`, the wiring, `shared/` and the data layer; the other framework's crate anywhere but the data layer |
| `rule-2`              | `sqlx` outside `repo.rs`/`model.rs`, the wiring, `shared/` and the data layer (`service.rs` may name `PgPool` and `PgConnection`) |
| `rule-4`              | a feature referring to another feature's modules                          |
| `rule-5`              | `shared/` referring to the features                                       |
| `rule-6`              | `unwrap()`, `expect()`, `panic!`, `todo!`, `println!`, `dbg!` outside tests |
| `cross-feature-write` | a feature calling another table's `<db_path>::<table>::write`             |
| `cross-feature-query` | a feature calling another feature's query module, `<db_path>::queries::<feature>` |
