# `rok` — CLI reference

The workspace's single binary. Contract text: this file is the user-facing
summary; the exact grammar lives in `crates/rok-cli/src/cli.rs` and — for
the `sqlc` subtree — the engine's `crates/rok-data-sqlc/src/cli.rs`, which
`rok` forwards to verbatim.

```text
rok generate [OPTIONS] [check]      model-crate generator
rok migrate <COMMAND>               runs migrations on the database
rok db <COMMAND>                    connectivity / structure of a database
rok schema <COMMAND>                live schema as DDL; journal-vs-files diff
rok sqlc <COMMAND>                  resource-module generator (engine)
```

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

## `rok generate` — whole-crate generation

```text
Usage: rok generate [OPTIONS] [COMMAND]
```

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

## `rok sqlc` — resource-module generator (delegated engine)

`rok sqlc …` is parsed by the engine's own clap grammar (`rok-data-sqlc`
is a library; `rok` forwards the tokens verbatim). Subcommand set:

| Command        | Alias | One-liner                                                                 |
| -------------- | ----- | ------------------------------------------------------------------------- |
| `generate`     | `g`   | parse the schema and emit `src/db/<table>/{…}` modules, optional api/openapi |
| `init`         | `i`   | scaffold a project (ADR 0016 skeleton or `--template actix`), files-if-absent |
| `check-arch`   | `a`   | check `src/` layout against the engine's ADR 0016 rules; violations → exit 1 |

Executive summary of the main flags (full text: `rok sqlc generate --help`):

| Flag                  | Meaning                                                              |
| --------------------- | -------------------------------------------------------------------- |
| `-s, --schema PATH`   | SQL file, or a migrations directory read by filename order; repeatable |
| `-o, --output DIR`    | output directory (default `src/db`)                                   |
| `-c, --config FILE`   | config file instead of discovered `rok-data-sqlc.toml`                |
| `-j, --json JSON`     | options as JSON (inline, `@FILE`, `-` for stdin)                      |
| `-a, --api-output DIR`| API feature layer (actix-web) into this directory                      |
| `-d, --derive <NAME>` | extra derives (`serde`, `validate`, …); `-e` engine features           |

`init` next steps and the `--template` list (`actix`) print on stdout;
the scaffolder never overwrites existing files. Templates ship their own
`README.md` with the dependency list to add.
