# rok-data

PostgreSQL data tooling for Rust, consolidated from the rok ecosystem:

- **runtime** — typed models over sqlx 0.9, a query builder, migrations
  with drift detection, and schema introspection;
- **two generators** — a whole-crate generator and a resource-module
  generator;
- **one CLI** — `rok`, the single user-facing binary, and one front door
  through it: `rok init` scaffolds a project, `rok gen` generates its data
  layer and API features from the project's schema directory.

## Install

```sh
cargo install rok-data-cli    # provides the `rok` binary
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

The front door: `rok init` scaffolds a project, `rok gen` generates its data
layer and its API features from the project's schema directory.

```console
$ rok init my-api --schema schema.sql
$ cd my-api && rok gen
```

`rok init` writes the project — the application skeleton, its settings file
and its schema directory (`migrations/`), seeded with the schema file you
name. `rok gen` turns that schema directory into the data layer
(`src/db/**`), the shared tools and the API features (`src/features/**`);
with `--migration NAME` it also stages the difference between the schema
and the last staged migration as a `NNNN_name.up.sql` / `NNNN_name.down.sql`
pair in the same directory. `rok gen check` is the CI mode: it writes
nothing and fails when the generated code is out of date.

A whole API in one step:

```console
$ rok init my-api --template axum    # a complete axum application
$ rok init my-api --template actix   # its actix-web twin
```

The whole chain in one crate: [`examples/api`](examples/api) holds a schema
of two files, the generated data layer and API features beside it, and the
staged migration pairs — the vertical slice the repository's own gates
compile.

Deriving the models by hand is one derive away:

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

The older generators still work — `rok generate` writes a whole model crate
from `.sql` files, `rok sqlc g` writes resource modules into an existing
crate — but they are **legacy**: superseded by `rok init`/`rok gen`, with
their removal deferred to 1.0.

Crate-by-crate docs: [docs/README.md](docs/README.md) (the map),
[docs/getting-started.md](docs/getting-started.md),
[docs/cli-reference.md](docs/cli-reference.md), the overview
([docs/overview.md](docs/overview.md)), and the per-crate
`//!` docs (`cargo doc --workspace --open`). Agents: start from
[docs/coding-agents.md](docs/coding-agents.md) and
[docs/llms.txt](docs/llms.txt).

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
