# ADR 0007: workspace standards — version, MSRV, lints, exclusions, CI

Date: 2026-10-19 · Status: accepted

## Decision

- **Version**: every crate shares the workspace version `0.1.0` until

  the ecosystem's first release policy is set.

- **MSRV**: `rust-version = "1.94"` for every crate (the floor the

  generated SQLx 0.9 code needs). rok-sqlc's standalone crate supported
  1.88; losing that floor is an accepted breaking change — engine and
  generated code now share one floor, which kept CI's matrix honest:

  `rust: ["1.94", stable]`.

- **Lints**: the workspace inherits `[lints]` with

  `missing_docs`, `missing_debug_implementations`, `unreachable_pub`,
  `unsafe_code` forbidden, `clippy::dbg_macro`/`todo` on warn. The
  rok-data-sqlc engine predates that set; because cargo forbids mixing
  `lints.workspace = true` with local overrides, its manifest spells out
  the workspace set minus `unreachable_pub` and `missing_docs` (its
  `pub` items inside private modules and its internal documentation
  granularity are part of its published style).

- **Exclusions**: fixture and example crates that manage their own

  dependency set (`tests/api-app`, `tests/fixture-app`,
  `tests/modules-app`, `docs/examples/*`, `templates/actix-api`) are
  excluded from the workspace and declare an empty `[workspace]`
  themselves, so cargo's discovery inside a workspace checkout cannot
  adopt them.

- **Line endings**: `* text=auto eol=lf` (`.gitattributes`); the ported

  trees were normalized to LF because the source clone was CRLF and the
  engine's own tests assert `\n` in generated/scaffolded output.

- **CI** (`.github/workflows/ci.yml`): fmt, clippy `-D warnings`, and
  the full workspace tests on 1.94 + stable for Linux (with PostgreSQL
  services enabling the DB-backed suites via `DATABASE_URL` for the
  rok-data-core suites and `ROK_DATA_SQLC_TEST_DATABASE_URL` for the
  engine's), and build + test on macOS/Windows stable without a database.

- **Stray-binary hygiene**: the engine's binary-driven integration

  tests compile the `rok` binary through `CARGO_BIN_EXE_rok` in
  `rok-cli/tests`, never a hand-built path.

## Amendment 2026-10-19 (adoption round, toasty study)

- **Deterministic expansion**: `rok-data-macros` carries a

  `clippy.toml` denying `HashMap`/`HashSet` — the `style` lint
  `disallowed_types` is warn-ranked, fires only when configured in
  `clippy.toml`, and is enforced by the existing `-D warnings`
  gate. Expansions must never depend on hash iteration order; use
  `BTreeMap`/`BTreeSet` (or `indexmap` when insertion order
  matters). No code changes were needed: the crate already used
  none.

- **Error model target**: new `rok-data-core` error-surface work

  follows the one-wrapper shape — pointer-sized, non-exhaustive
  `Error`, one module per kind with a constructor, an `is_*()`
  predicate, and `.context()` chaining — with `Result<T, E = Error>`
  as the alias. This is a design target recorded in
  `docs/unified-pipeline-plan.md`'s backlog (item 13), not a rewrite
  of existing surfaces.

- **Docs pipeline** (guide pages doctested from a build script,

  "Runnable example" pointers per page, CI running examples by
  glob): adopted as the plan's docs phase (backlog item 14).

- **Release tooling**: ADR 0008's tag == version, checksummed

  bundles, and CHANGELOG-heading gate stay; `release-plz` is a
  deliberate no for 0.2.0 (backlog note).

- Already in force before this amendment (verified, not adopted):

  MSRV discipline (`rust-version = "1.94"`, CI matrix
  `["1.94", "stable"]`, asserted by the `release_metadata` suite)
  and `unsafe_code = "forbid"`.
