# ADR 0003: the SQLc engine is a library; binary-driving tests live with the CLI

Date: 2026-10-19 · Status: accepted

## Context

rok-sqlc shipped a library and a thin `main.rs` binary around the same
clap parser. Three facts collide once it moves into the workspace:

1. The workspace has exactly one binary (`rok`, ADR 0002), so the

   engine's binary target goes away.

2. `env!("CARGO_BIN_EXE_rok-data-sqlc")` only exists for integration

   tests *inside the crate that defines that binary target*; with the
   target gone, 13 references across its suites stop compiling.

3. The engine's CLI grammar is its most-tested surface; duplicating it

   into rok-cli would fork the truth.

## Decision

- The engine keeps its `cli` module (hidden from the public-facing API

  narrative but exported) with `run`, `run_from` and `command`. rok-cli

  routes `rok sqlc …` to `run_from` verbatim.

- The engine's test suites that drove the binary move from

  `rok-data-sqlc/tests/{cli,stale_and_check,fixture_compile,conventions}`
  plus `api_openapi.rs` and `readme.rs` into `rok-cli/tests/…`. They now
  spawn `CARGO_BIN_EXE_rok` (the workspace binary) with the `sqlc`
  subcommand prepended — they exercise the *user's* path end to end.

- In-process helpers remain in the moved suites (the harness in
  `rok-cli/tests/cli/main.rs` calls `rok_data_sqlc::cli::run_from`
  directly); suites with no binary references stay in the engine crate.

- Fixtures do **not** move: `tests/api-app`, `tests/fixture-app`,
  `tests/modules-app`, `modules/`, `templates/actix-api`,
  `docs/examples/*` stay in the engine crate. The moved suites resolve
  them through `rok-cli/tests/common/mod.rs`'s
  `sqlc_manifest_dir()` helper (sibling directory of rok-cli), keeping
  the repository layout stable for `git history` and for the fixture
  crates' own rust-version declarations.

## Consequences

- The engine crate and rok-cli both build the same fixture crates into a

  shared `target/fixture-target`, so the expensive compile happens once.

- The moved suites are the *only* place in the workspace that exercises

  the `rok` binary end to end; rok-cli adds a few focused tests of its
  own (help/version/exit codes).

- `init --template`'s skip predicate and the writer recognize legacy

  rok-sqlc markers (ADR 0005), which the moved tests directly assert.
