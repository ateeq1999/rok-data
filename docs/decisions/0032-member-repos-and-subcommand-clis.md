# ADR 0032: rok-data splits along tool lines into member repos

Date: 2026-11-02 · Status: accepted (amends ADR 0002, ADR 0022 and ADR 0026)

## Context

rok-data was assembled from four repositories because separate repos
drifted and shipped three overlapping binaries (ADR 0002). It has since
grown templates for web, mobile and desktop that make up 47% of its
tracked files and bring bun, Flutter and desktop toolchains, while the
`rok` binary name also belongs to an older runner of ours. The goal is a
light rok-data and a family of small command lines under one `rok`.

## Decision

- **The split follows tool lines, not crate lines.** The runtime, the
  codegen and the sqlc engine stay in one repo with their CLI and its
  tests: the compile gates that catch a runtime change breaking
  generated code need both sides in one commit — the drift ADR 0002
  consolidated against.

- **Member repos**: `rok-data` (runtime, codegen, engine, and its
  Subcommand CLIs), `rok-templates` (the templates and their gates),
  `rok-tools`, `rok-ui`, and `rok-v2`, which owns the `rok` binary
  (published as the `rok-cli` crate from 0.7.0) and nothing but routing,
  help and the project registry (ADR 0028). Amends ADR 0002: rok-data's
  binary becomes `rok-data`, then `rok-db` and `rok-gen` when the CLI
  splits.

- **`rok` dispatches two ways**: linked Subcommand CLIs (`command()` and
  `run_from(args)`, compiled in, the pattern ADR 0026 set for rok-tools),
  and, for any other word, `rok-<word>` found on the `PATH`. Every
  Subcommand CLI keeps ADR 0002's exit codes and `--json` contract and
  reads only its own section of `rok.toml`.

- **The templates leave rok-data** (amends ADR 0022's location). The
  engine keeps depending on the `rok-templates` crate, now from
  crates.io, so `rok sqlc init --template` stays engine grammar
  (invariant 3). rok-templates' gates generate with a pinned
  `rok-data-cli`; an engine change to generated template code reaches
  the templates when that pin moves.

- **A meta-repo, `rok-workspace`**, is the folder the member repos are
  checked out in: their list, a map for agents, and a `[patch.crates-io]`
  file passed explicitly (`--config`) so unreleased changes across repos
  build together. A nightly job builds `rok-v2` against every member's
  `main`.

## Considered Options

- **Everything stays in rok-data**: the templates' toolchains and size
  stay in every checkout and every agent's context.
- **The CLI moves to `rok-v2` whole**: the engine's CLI suites
  (invariant 4) would leave the engine's repo, so an engine change could
  not be tested where it is made.
- **Git submodules**: pinned commits, and every change pushed twice.
- **The patch file in `.cargo/config.toml`**: found automatically, it
  would silently apply to every build under the folder, release checks
  included.

## Consequences

- Each repo releases on its own semver; order is rok-data and rok-tools,
  then rok-templates, then rok-v2.
- `cargo install rok-cli` switches from the old task runner (0.6.1) to
  the umbrella at 0.7.0.
- The plan is `docs/specs/toolchain-split/plan.md`, moved to
  `rok-workspace` when it exists.

*Amended 2026-11-03, at the CLI split:* `rok-db` and `rok-gen` are two
more binaries of the one `rok-data-cli` crate (`run_group` with a
`Group`), not the crates `rok-db-cli` and `rok-gen-cli` the plan named.
The two halves share `config`, `context`, `keychain` and `output`, so two
crates would have needed a third, shared one: three new crate names
against crates.io's new-crate limit, for halves that release together
anyway (one repository, one version). The binary names are the contract,
so the crates can still split later without breaking anyone.
