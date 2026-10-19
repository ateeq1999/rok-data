# Migration plan

The consolidation was done in verification-first phases; each phase was
green (or its failures reported as unskippable) before the next started.

## Phase 1 — audit

Read all four repositories and their tests; record what each actually
does, not what its name implies. Findings in
[migration-audit.md](migration-audit.md). No merge decisions were made
before this.

## Phase 2 — workspace scaffold

- New git repository `rok-data` that **contains** everything; no
  workspace members point outside the repository.
- Root `Cargo.toml`: members, shared `version = 0.1.0`, edition 2024,
  `rust-version = 1.94`, MIT OR Apache-2.0, `[profile.release] strip =
  true`.
- `.cargo/config.toml` (Windows MSRV fallback resolver setting),
  `rustfmt.toml` (`edition = "2024"`), `.gitattributes`
  (`* text=auto eol=lf`), `.gitignore`.
- Fixture and example crates that use their own dependency sets are
  declared in `workspace.exclude` and carry their own `[workspace]`
  tables (`tests/{api-app,fixture-app,modules-app}`,
  `docs/examples/*`, `templates/actix-api`).

## Phase 3 — port rok-db runtime

- `crates/rok-data-core`, `rok-data-macros`, `rok-data` (facade),
  `rok-data-codegen`, and `examples/sqlgen/db-gen` + `sqlgen` runner,
  ported with the mechanical rename `rok-db*` → `rok-data*`,
  `rok_db` → `rok_data`, journal `_rok_db_migrations` →
  `_rok_data_migrations`, config `rok-data.toml` with
  `[gen] rok_data_version`/`rok_data_path`.

- The codegen crate lost its `[[bin]]`; regenerating the example is
  `cargo run -p rok-data-codegen --example regenerate`.

- Verified: the example regenerates byte-identical,
  `cargo test --workspace --all-targets` + doc tests green.

## Phase 4 — port rok-sqlc engine

- `crates/rok-data-sqlc`, ~30k lines: manifest converted to workspace

  inheritance; `ROK_SQLC_*` env vars → `ROK_DATA_SQLC_*`; settings file
  `rok-sqlc.toml` → `rok-data-sqlc.toml`; the embedded `init` templates
  (which still ship files generated before the rename) keep their
  content verbatim.

- Lints: the engine predates the workspace lint set; instead of

  inheriting it wholesale it mirrors the workspace lints minus
  `unreachable_pub`/`missing_docs` (cargo does not allow overriding
  inherited lints, so the table is spelled out; see
  `crates/rok-data-sqlc/Cargo.toml`).

- Its own CLI binary target was removed; the clap grammar stays in the

  library (`cli` module) for rok-cli to drive.

- The `100+ module corpus (modules/)` and the fixture crates were
  copied; workspace exclusion solved cargo's "believes it's in a
  workspace" discovery error; template/example files normalized to LF
  (the source clone was CRLF).

## Phase 5 — rok-cli

- `crates/rok-cli`, one binary `rok`: `generate`, `migrate`,
  `db`, `schema`, `sqlc`. Exit codes: help/version → 0; root usage
  errors → 2; usage errors inside the delegated sqlc subtree → 1 (the
  engine's historic behavior).
- `--json` flags for migrate status/up/down and db check with a stable
  machine contract (`output::json_line`, `StatusRow` JSON).
- Tests: the binary-driving engine suites relocated to `rok-cli/tests`
  (ADR 0003), plus in-process CLI tests.

## Phase 6 — compatibility machine

- Marker recognition (legacy + current) in writer, init templates,
  test helpers.
- File migration loader accepts rok-fluent single-file format.
- CI at the root (`.github/workflows/ci.yml`): fmt, clippy `-D
  warnings`, tests on 1.94 + stable, with PostgreSQL services enabling
  the DB-backed suites.

## Phase 7 — documentation & final report

This documents/ tree, README + AGENTS.md, CLI reference, migration
guide, benchmarks, and the closing report (see the chat summary).

## What is deliberately out of scope (recorded as remaining issues)

- ~~A release/publish workflow for the consolidated workspace~~ — since
  designed and implemented: see
  [ADR 0008](decisions/0008-release-policy.md) and
  `.github/workflows/release.yml` (tag-driven binary bundles with
  checksums and install scripts, ordered crates.io publishing).
- rok-fluent/rok-orm's APIs are documented as mappings only; no
  compatibility shims are provided.
- The engine's `api-*` suites that need `DATABASE_URL` were ported and
  print skip notes without one, like the original.
