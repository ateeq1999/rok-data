# `rok` — CLI reference

The workspace's single binary. Contract text: this file is the user-facing
summary; the exact grammar lives in `crates/rok-cli/src/cli.rs` and — for
the `sqlc` subtree — the engine's `crates/rok-data-sqlc/src/cli.rs`, which
`rok` forwards to verbatim.

```text
rok init [NAME]                   scaffold a project (skeleton or template)
rok gen [OPTIONS] [check]         data layer and API features from the schema dir
rok generate [OPTIONS] [check]      legacy: model-crate generator
rok migrate <COMMAND>               runs migrations on the database
rok db <COMMAND>                    connectivity / structure of a database
rok schema <COMMAND>                live schema as DDL; journal-vs-files diff
rok sqlc <COMMAND>                  legacy: resource-module generator (engine)
```

Two commands are **legacy**: `rok generate` and `rok sqlc …`. They keep
working exactly as they always have — the `sqlc` tokens are forwarded
verbatim to the engine — but they are superseded by the front door (`rok gen`
generates, `rok init` scaffolds), and their removal is deferred to 1.0.

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
  - `db check --json` reports `{"connected": true, "server_version": "..."}`
    (or the reason it could not connect).
- Exit codes: `--help`/`--version` = 0; usage errors at the root = 2
  (clap); runtime failures (bad URL, drift, check failures) = 1. Inside
  `rok sqlc …`, usage errors exit 1 — the engine's own historic behavior,
  preserved so its tests keep asserting it.
- Flags beat the settings file wherever both exist.

## `rok init` — scaffold a project

```text
Usage: rok init [OPTIONS] [NAME]
```

Scaffolds a project: the application skeleton (`main`/`app`/`config`/`error`/
OpenAPI wiring, ADR 0016), or — with `--template` — a ready-made **Template**,
a complete starter project for one web framework. The scaffolding itself is
the engine's (`rok_data_sqlc::init`); the root command owns the grammar, the
schema seeding and the report.

| Flag                  | Meaning                                                            |
| --------------------- | ------------------------------------------------------------------ |
| `-t, --template NAME` | copy this Template instead of the skeleton (`actix` or `axum`)      |
| `-s, --schema PATH`   | seed the schema directory (`migrations/`) with this file's content |

`NAME` is the project directory: it is created when missing, and the current
directory is the project when `NAME` is left out. Every file it writes is a
**scaffolded file**: written once, never rewritten. A second run only fills in
what is missing; a file the user has edited is kept as it is. Cargo's
untouched default `src/main.rs` (the one `cargo new` writes) is replaced, and
the run says so; any other `main.rs` is kept.

`--schema PATH` copies that file to `migrations/0001_init.sql`, so the schema
directory — the project's only source of truth — starts non-empty. An
existing `migrations/0001_init.sql` is kept, never overwritten, and the run
says that it kept it.

The run prints what it wrote and the next steps, which name the front door:
`rok gen` turns the schema directory into `src/db/` and `src/features/`.

An unknown `--template` is a usage error (exit 2) and names the valid
templates. A failure while scaffolding (an unwritable directory, an
unreadable `--schema` file) prints `error: …` on stderr and exits 1.

## `rok gen` — the front door: the data layer from the schema dir

```text
Usage: rok gen [OPTIONS] [COMMAND]
```

Two things, both over the project's schema dir: the data layer
(`src/db/**`), the shared tools and the API features (`src/features/**`)
come from the engine's generator, and `--migration NAME` stages a
migration pair with the codegen crate's diff engine. The schema dir is the
project's only source of truth (ADR 0010).

| Flag                       | Meaning                                                            |
| -------------------------- | ------------------------------------------------------------------ |
| `-c, --config FILE`        | settings file (`rok.toml` `[generate]` table; the engine's `rok-data-sqlc.toml` also accepted) |
| `-s, --schema DIR`         | the schema dir (default: the config's `schema`, else `migrations/`) |
| `-o, --output DIR`         | where the modules are written (default `src/db`)                    |
| `-r, --resource TABLE`     | generate one table plus the resources already generated             |
| `-q, --queries PATH`       | annotated query file or directory; repeatable                       |
| `--migration NAME`         | stage a migration pair for the schema changes since the last one    |
| `--allow-destructive`      | allow the steps that lose data or fail on existing rows             |
| `--rename OLD=NEW`         | `users=accounts` (table) or `users.name=full_name` (column); repeat|
| `--strict`                 | fail on warnings (a skipped table or query)                        |
| `--json`                   | one JSON object on stdout                                           |
| `-n, --dry-run`            | plan the run; write nothing                                         |
| `-f, --force`              | overwrite files the generator did not write                          |

The settings file: `rok.toml` (the canonical name, ADR 0011) holds the
generation settings in its `[generate]` section, beside `[api]` and
`[conventions]`; the engine's `rok-data-sqlc.toml`, whose keys sit at the
top level, is read as well. Flags beat the file.

The `[api]` section holds the API features' settings. Its `framework` key
picks the web framework they are generated for: `actix_web` (the default)
or `axum`; `--api-framework` sets it from the command line. The framework
needs its extractor in `extractors` (`actix_web` or `axum`).

The schema dir holds the **truth** (plain `*.sql` files) and the
**history** (the staged `NNNN_name.up.sql` / `NNNN_name.down.sql` pairs).
The generator reads the truth only, so a staged migration is never applied
twice; the staged pairs use the runtime's `NNNN_name.up.sql` convention.

`--migration NAME` diffs the truth against the state of the last staged
migration (the snapshot `.rok-schema.json` in the schema dir) and writes
`NNNN_NAME.up.sql` plus — when the steps can be undone —
`NNNN_NAME.down.sql` into the schema dir. An existing migration is never
overwritten, and the model crate is never written. A step that loses data
(a dropped column or table, a new `NOT NULL` column without a default) is
refused unless `--allow-destructive` is given; `--rename` records a
rename instead of a drop and an add.

The run prints what it wrote, what it skipped and what it refused, and the
numbered next steps. `--json` prints one object instead:
`{"written": [...], "scaffolded": [...], "unchanged": [...], "refused":
[{"path": ..., "reason": ...}], "stale": [...], "missing": [...],
"changed": [...], "skipped": [{"kind": "table"|"query", "name": ...,
"reason": ...}], "warnings": [...], "migration": {"version": ..., "name":
..., "up": ..., "down": ..., "reversible": ..., "steps": [...]}, "drift":
<bool>, "dry_run": <bool>}`.

### `rok gen check [OPTIONS]`

The CI mode: nothing is written, the missing, changed and stale files are
reported, and the run exits 1 when there is drift (`--json` reports the
drift and exits 1 as well). The same flags minus `--migration` and
`--rename`, which belong to the generate run: given to `rok gen` together
with `check` they are an error, given to `rok gen check` they are a usage
error. The leaf's flags win over the group's; the booleans OR.

## `rok generate` — whole-crate generation (legacy)

```text
Usage: rok generate [OPTIONS] [COMMAND]
```

Legacy: superseded by `rok gen`, the front door's generator. The command
keeps working exactly as it does now — same flags, same output, same exit
codes — and its removal is deferred to 1.0.

The write mode keeps the original `rok-db-gen` flat shape; the CI mode
nests as `rok generate check`.

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
`docs/unified-pipeline-plan.md`) reconciles the two.

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

## `rok db` — connectivity and structure

```text
rok db check   [--database-url] [--json]  connect, ping, server version, pool settings
rok db inspect [--database-url] [--json] [--table T]  user tables; or one table's
                                          columns (name, type, length, nullability,
                                          default, is_pk), indexes and foreign keys
```

Both commands use `information_schema`/`pg_catalog` via
`rok-data-core::introspect`.

## `rok schema` — live schema, and journal consistency

```text
rok schema dump [--database-url]       approximate CREATE TABLE DDL for every user table
rok schema diff [--database-url] [--dir migrations]
                                       compares the journal's applied migrations with
                                       the files in --dir (missing, extra, drifted)
```

`diff` is the CI guard: exit 1 when journal and files disagree.

## `rok sqlc` — resource-module generator (delegated engine, legacy)

Legacy: superseded by the front door — `rok gen` for the generation,
`rok init` for the scaffolding. The tokens are still forwarded verbatim to
the engine, so the command keeps working exactly as it does now; its
removal is deferred to 1.0.

`rok sqlc …` is parsed by the engine's own clap grammar (`rok-data-sqlc`
is a library; `rok` forwards the tokens verbatim). Subcommand set:

| Command        | Alias | One-liner                                                                 |
| -------------- | ----- | ------------------------------------------------------------------------- |
| `generate`     | `g`   | parse the schema and emit `src/db/<table>/{…}` modules, optional api/openapi |
| `init`         | `i`   | scaffold a project (ADR 0016 skeleton or `--template actix`/`axum`), files-if-absent |
| `check-arch`   | `a`   | check `src/` layout against the engine's ADR 0016 rules; violations → exit 1 |

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

`init` next steps and the `--template` list (`actix`, `axum`) print on stdout;
the scaffolder never overwrites existing files. Templates ship their own
`README.md` with the dependency list to add.
