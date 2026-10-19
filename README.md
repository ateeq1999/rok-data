# rok-data

PostgreSQL data tooling for Rust, consolidated from the rok ecosystem:

- **runtime** — typed models over sqlx 0.9, a query builder, migrations
  with drift detection, and schema introspection;
- **two generators** — a whole-crate generator and a resource-module
  generator;
- **one CLI** — `rok`, the single user-facing binary.

## Install

```sh
cargo install rok-cli        # provides the `rok` binary
```

Or take a prebuilt `rok` binary: the GitHub releases carry checksummed
archives for Linux (musl), macOS, and Windows, x86_64 and ARM64, and the
repository's `install.sh` / `install.ps1` download, verify and install
them:

```sh
curl -fsSL https://raw.githubusercontent.com/ateeq1999/rok-data/main/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/ateeq1999/rok-data/main/install.ps1 | iex
```

(See [ADR 0008](docs/decisions/0008-release-policy.md) for the whole
release story.) The crates need Rust 1.94 or newer.

## Quick start

```console
$ cargo new my-app && cd my-app
$ cargo add rok-data
```

```rust,ignore
use rok_data::prelude::*;

#[derive(Model)] // rok_data::macros
struct Note { id: Uuid, text: String }

// migrations/0001_notes.up.sql + .down.sql pair, journal-managed
```

Apply it:

```console
$ rok migrate up        # reads DATABASE_URL, writes the _rok_data_migrations journal
```

Generate the typed models from SQL instead of writing derives by hand:

```console
$ rok generate --input sql --output . --crate-name my-app-db
```

Or, for resource-module style code inside your crate:

```console
$ rok sqlc g --schema schema.sql
```

Crate-by-crate docs: [docs/getting-started.md](docs/getting-started.md),
[docs/cli-reference.md](docs/cli-reference.md), and the per-crate
`//!` docs (`cargo doc --workspace --open`).

## The workspace

| Crate                     | One-liner                                       |
| ------------------------- | ----------------------------------------------- |
| [`rok-data`](crates/rok-data) | the facade applications depend on            |
| [`rok-data-core`](crates/rok-data-core) | `Db`, builders, migrations, introspect |
| [`rok-data-macros`](crates/rok-data-macros) | derives + `#[rok::test]`      |
| [`rok-data-codegen`](crates/rok-data-codegen) | whole-crate generator       |
| [`rok-data-sqlc`](crates/rok-data-sqlc) | resource-module generator engine  |
| [`rok-cli`](crates/rok-cli) | the `rok` binary                               |

Browse from [docs/architecture.md](docs/architecture.md), read the
[migration audit](docs/migration-audit.md) for why this workspace exists,
and the [decisions/](docs/decisions/) ADRs for every deliberate choice.

## Development

```console
$ cargo fmt --all
$ cargo clippy --workspace --all-targets -- -D warnings
$ cargo test --workspace --all-targets
```

Database-backed suites need `DATABASE_URL` / (engine)
`ROK_DATA_SQLC_TEST_DATABASE_URL`; without one they print a skip note
and pass, exactly like the original repositories. CI runs them with
PostgreSQL 17 services.
