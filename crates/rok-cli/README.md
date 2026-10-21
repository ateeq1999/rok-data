# rok-data-cli

The `rok` binary — the single command-line interface of the
[rok-data](https://github.com/ateeq1999/rok-data) workspace. Everything it
does is backed by the workspace's libraries (`rok-data`,
`rok-data-codegen`, `rok-data-sqlc`), so scripts can call those crates
directly; this crate is the thin shell that routes arguments and formats
output.

```sh
cargo install rok-data-cli
rok --help
```

The package installs under `rok-data-cli` (`rok-cli` is taken on
crates.io); the binary it installs is `rok`.

```text
rok init [NAME]                   scaffold a project (skeleton or template)
rok gen [OPTIONS] [check]         data layer and API features from the schema dir
rok generate [OPTIONS] [check]      legacy: model crate from .sql files
rok migrate <COMMAND>               create|status|up|down (+ rollback)
rok db <COMMAND>                    check|inspect the configured database
rok schema <COMMAND>                dump|diff the live schema and journal
rok sqlc <COMMAND>                  legacy: resource-module generator (g|i|a)
```

Most commands read the database from `DATABASE_URL` (or
`--database-url`), PostgreSQL 12+, and offer `--json` for a stable
machine-readable contract. The `rok sqlc …` subtree is the
`rok-data-sqlc` engine's own clap grammar, forwarded verbatim — so its
tests drive the `rok` binary exactly as a user would.

`rok generate` and `rok sqlc …` are the legacy surfaces: still supported,
still forwarded verbatim, superseded by the front door (`rok init`,
`rok gen`), and their removal is deferred to 1.0.

The full command contract (flags, defaults, exit codes, JSON shapes):
[docs/cli-reference.md](../docs/cli-reference.md). Architecture and the
reasons behind this crate's shape: [docs/architecture.md](../docs/architecture.md).

License: MIT OR Apache-2.0.
