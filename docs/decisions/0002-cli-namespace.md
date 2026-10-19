# ADR 0002: one CLI namespace, the `rok` binary

Date: 2026-10-18 · Status: accepted

## Context

The four repositories shipped three separate user-facing binaries
(`rok-db-gen`, `rok-fluent`'s CLI, `rok-sqlc`). Consolidating without a
CLI decision would leave three overlapping tools — worse than before.

## Decision

- `crates/rok-cli` builds the single binary `rok`. No other workspace
  crate has a binary target (the example's regeneration is
  `cargo run -p rok-data-codegen --example regenerate`).

- Command set:

  `generate`, `generate check`, `migrate create|status|up|down`
  (`rollback` alias for `down`), `db check|inspect`,
  `schema dump|diff`, `sqlc generate|init|check-arch`.

- `rok sqlc …` is not re-parsed: tokens are forwarded verbatim to

  `rok_data_sqlc::cli::run_from`, so the engine's tested clap grammar,
  help text, aliases (`g|i|a`) and exit codes stay single-source. The
  root help embeds the engine's `sqlc` subcommand definition
  (`Cli::command().subcommand(engine::command().name("sqlc"))`) purely
  so `rok --help` lists it.

- The recorded defaults: `rok schema diff --dir migrations`;
  `--json` makes migrate/db commands print the stable machine lines and
  nothing else on stdout.

- Exit codes: help/version 0; root usage errors 2; usage errors inside

  the delegated `sqlc` subtree 1 (the engine's historic behavior);
  runtime failures 1.

## Consequences

- Users migrate by replacing `rok-db-gen migrate status` with

  `rok migrate status` (etc.); `rok-sqlc g` becomes `rok sqlc g`.

- The engine crate cannot have a binary target — its CLI tests that

  spawned `CARGO_BIN_EXE_rok-data-sqlc` had to move (ADR 0003).
