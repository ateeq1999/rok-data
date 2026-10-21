# ADR 0008: release policy — one version, one tag, ordered publishing

Date: 2026-10-19 · Status: accepted

## Context

The standalone rok-sqlc repository shipped a release pipeline
(buffered binary archives per-platform, checksummed, published by tag,
install scripts) and released one crate. The workspace has **one binary
and six publishable crates** and inherits no shared release story.
Without one, the CLI's distribution breaks for the users the original
install scripts served.

## Decision

(`.github/workflows/release.yml`, `install.sh`, `install.ps1`, root
`CHANGELOG.md`)

- **One version**: every crate inherits `[workspace.package] version`.
  A tag `vX.Y.Z` must equal that version; the release workflow fails
  any mismatch.
- **Notes**: release notes come from root `CHANGELOG.md`'s
  `## [X.Y.Z]` section; a missing section fails the release (Keep a
  Changelog style).
- **Binaries**: `cargo build --workspace --release` on six targets —

  x86_64/aarch64 for Linux (musl), macOS (darwin), Windows (msvc) —
  packaged as `rok-<version>-<target>.tar.gz` / `.zip` with a `.sha256`
  sidecar and a combined `SHA256SUMS`, each bundle carrying README,
  CHANGELOG, and both license files from the root.
- **Crate publishing**: sequentially, in dependency order
  (`rok-data-macros`, `rok-data-core`, `rok-data`,
  `rok-data-codegen`, `rok-data-sqlc`, `rok-data-cli` — the CLI crate
  last so `cargo install rok-data-cli` resolves the published set). Crates
  whose version is already on crates.io are skipped, supporting the "run by
  hand again" path.
- **Self-check** (`verify-install`): the shipped `install.sh` /
  `install.ps1` install on Ubuntu/arm/macOS/Windows runners and the
  installed `rok --version` must report the released version.
- **Install script environment**: `ROK_VERSION`, `ROK_INSTALL_DIR`,
  `ROK_BASE_URL`, `ROK_NO_MODIFY_PATH` (the engine's `ROK_SQLC_*`
  names, renamed). The scripts live at the repository root so users can
  `curl | sh` them from raw.githubusercontent.com/ateeq1999/rok-data/main.

## Consequences

- **TLS**: the workspace's sqlx carries no TLS stack (`default-
  features = false`). The `rok` binary therefore cannot connect to
  endpoints that insist on TLS (`sslmode=require`); this matches the
  original tools' behavior and keeps musl builds statically linked.
  Adding TLS is a `Cargo.toml` feature change, not a pipeline change.
- **crates.io names**: availability of `rok-data`, `rok-data-cli`, etc.
  is a pre-tag task (checked before v0.2.0: `rok-cli` was taken, so the
  CLI publishes as `rok-data-cli`).
- **Repo hygiene**: `rok-data-sqlc`'s own `LICENSE-MIT` now names
  "The rok-data contributors"; the root licenses ship in every bundle.
- A first failed run will surface the repo's org/GitHub-name
  assumptions (`ateeq1999/rok-data`) — the one thing CI cannot dry-run.
