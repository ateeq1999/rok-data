---
title: Installation
description: Get the rok-data toolchain and runtime on any platform — CLI, runtime crate, and the pieces the family expects.
updated: 2026-10-29
kind: guide
source: site/ (installation page), README.md, Cargo.toml (rust-version)
---

# Installation

The rok-data toolchain in minutes: one binary (`rok`), one runtime crate
(`rok-data`), one PostgreSQL server.

**Prerequisites**

- Rust **1.94** or later (`rustc --version`) — edition 2024.
- A PostgreSQL **12+** server for anything that touches a database
  (generation itself does not need one).
- Ledger of names: the CLI crate publishes as **`rok-data-cli`**; the
  binary it provides is **`rok`**. (`cargo install rok-cli` installs the
  *platform family's* CLI — a different project.)

## Install the CLI

From crates.io:

```sh
cargo install rok-data-cli        # provides the `rok` binary
```

Or from a checkout of this repository:

```sh
cargo install --path crates/rok-cli
```

Or take a prebuilt binary — GitHub releases carry checksummed archives
for Linux (musl), macOS, and Windows, x86_64 and ARM64, and the
repository's installers download, verify and install them:

```sh
curl -fsSL https://raw.githubusercontent.com/ateeq1999/rok-data/main/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/ateeq1999/rok-data/main/install.ps1 | iex
```

## Verify

```sh
rok --version
```

Should print `rok 0.3.0` or later. A one-command smoke test that needs no
database:

```sh
rok --help
```

## Add the runtime

For a new application crate:

```sh
cargo new my-app && cd my-app
cargo add rok-data
```

The `rok-data` facade is the only dependency applications need: it
re-exports the runtime (`rok-data-core`) and the macros (`rok-data-macros`
— `#[derive(Model)]`, the other derives, `#[rok_data::test]`). Optional
facade features ride the columns you use (`chrono`, `uuid`, `decimal`,
`json`, `serde`, `axum`, `metrics`, `ipnetwork`, `mac_address`, …); see
the workspace features table at the end of
[`docs/feature-matrix.md`](feature-matrix.md).

Point the toolchain at a database through the connection string
(`--database-url` beat the environment variable `DATABASE_URL`; URLs are
printed masked everywhere):

```sh
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/my_app
```

## Platform notes

**Windows.** The toolchain links PostgreSQL's socket code as any Rust
crate does; you need the Microsoft C++ Build Tools — grab them from
[Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/),
then `cargo` from a fresh terminal. For a PostgreSQL server, any
installer works (the scoop/chocolatey packages are common).

**macOS.** Make sure Xcode Command Line Tools are installed:

```sh
xcode-select --install
```

(And `brew install postgresql@17` when you need a local server.)

**Linux.** Build dependencies only; the runtime needs none:

```sh
# Ubuntu/Debian
sudo apt-get install build-essential pkg-config libssl-dev

# Fedora
sudo dnf install pkg-config openssl-devel

# Arch
sudo pacman -S pkg-config openssl
```

## Next steps

- [`docs/getting-started.md`](getting-started.md) — scaffold a project
  (`rok init`), generate its data layer (`rok gen`), apply migrations.
- [`docs/coding-agents.md`](coding-agents.md) — drive the toolchain from a coding agent
  (JSON contracts, exit codes, CI gates).
- [`docs/cli-reference.md`](cli-reference.md) — every command, flag and
  exit code.
