# Queries checked against the database, and a polling watch

Date: 2026-10-29 · Status: accepted

The engine types every annotated query from the schema files alone: a parameter takes the type of the column it is compared with, and a result column the type of the column it reads. That covers most queries, but not all of them. An expression, a cast the analysis reads differently, a `-- param:` override that is wrong, or a database that has drifted from the schema files can each give a function that compiles and then fails at run time with a decode error. This ADR adds two commands: `rok sqlc check --db`, which catches those mismatches before the application runs, and `rok sqlc generate --watch`, which shortens the loop that finds them.

**`check --db` prepares; it never runs.**

- **What it does:** every analyzed query is prepared against `DATABASE_URL` (or `--db URL`) inside a transaction that is rolled back. PostgreSQL answers the parameter and column types, and the column nullability where it can tell.
- **What it compares:** each parameter's type with the type the generated function binds, and each result column's name, type and nullability with what the generated code decodes. A column the database says can be `NULL` that decodes into a non-`Option` type is a mismatch. A column whose nullability the database can't tell is not.
- **What it reports:** each mismatch as `file:line: Query: message`, and exit 1 if there is any. A Rust type the check does not know, such as a user's own `-- param:` type, is a note on stderr, not a failure, so a project with custom types can still use the check in CI.
- **What comes first:** `check`, with or without `--db`, first checks that the generated code is up to date, as `generate --check` does. With both checks one command, CI needs a single step.

**The engine keeps no database driver.**

- **The seam:** the engine defines a `Describer` trait (`check_db`). The `rok` binary implements it over the runtime's new `rok_data::introspect::describe` and hands it to the engine (`cli::run_with`).
- **Why:** sqlx stays a dependency of `rok-data-core` only, as the workspace requires. The grammar of `rok sqlc check` stays the engine's, so its tests verify the surface users see. The engine's `run_from` has no describer, and `check --db` there says to use the `rok` binary.

**`--watch` polls.**

- **How it works:** the inputs are the config file and the configured schema and query paths. Every 250 ms, their files' sizes and modification times are compared with the last look. A change waits until two looks agree, so a save or a branch switch regenerates once.
- **What it prints:** the first run prints the whole report. Each later run prints its failures and warnings, then one line. A failed run does not end the watch.
- **What it never does:** apply migrations. That stays an explicit `rok migrate up`.

## Considered Options

- **The engine depends on sqlx, or on `tokio-postgres`.** Rejected: it would be a second database driver in the workspace, and a library that today only reads and writes files would gain network access.
- **`check --db` in `rok-cli` as a root command.** Rejected: the `rok sqlc` grammar lives in the engine (workspace AGENTS invariant 3).
- **Executing each query with sample values.** Rejected: it runs writes, it needs data, and preparing already reports every type.
- **A platform file watcher (`notify`).** Rejected: it is a dependency with platform backends, for a handful of small files that a poll reads in microseconds. The 250 ms interval also serves as the debounce.
- **Failing on unknown types.** Rejected: a project's own types would make the check unusable in CI.

## Consequences

- `rok-data-core` gains `introspect::describe(db, sql) -> Statement`, a stable runtime API: the parameter type names, and each column's name, type name and nullability.
- The engine's `cli` gains `run_with(args, connect)` and the `Connect` type, beside `run_from`.
- A check is only as good as the database it runs against: run it after `rok migrate up`, against a database migrated like production.
- Nullability is checked only where PostgreSQL reports it (table columns and outer joins). An expression's nullability is left to the generated code.
