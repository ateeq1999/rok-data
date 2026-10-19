# rok-cli

The `rok` binary — the single command-line interface of the
[rok-data](https://github.com/ateeq1999/rok-data) workspace. Everything it
does is backed by the workspace's libraries (`rok-data`,
`rok-data-codegen`, `rok-data-sqlc`), so scripts can call those crates
directly; this crate is the thin shell that routes arguments and formats
output.

```sh
cargo install rok-cli
rok --help
```

```text
rok generate [OPTIONS] [check]      model crate from .sql files
rok migrate <COMMAND>               create|status|up|down (+ rollback)
rok db <COMMAND>                    check|inspect the configured database
rok schema <COMMAND>                dump|diff the live schema and journal
rok sqlc <COMMAND>                  resource-module generator (g|i|a)
```

Most commands read the database from `DATABASE_URL` (or
`--database-url`), PostgreSQL 12+, and offer `--json` for a stable
machine-readable contract. The `rok sqlc …` subtree is the
`rok-data-sqlc` engine's own clap grammar, forwarded verbatim — so its
tests drive the `rok` binary exactly as a user would.

The full command contract (flags, defaults, exit codes, JSON shapes):
[docs/cli-reference.md](../docs/cli-reference.md). Architecture and the
reasons behind this crate's shape: [docs/architecture.md](../docs/architecture.md).

License: MIT OR Apache-2.0.
