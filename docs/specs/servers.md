# Spec — Servers: per-user PostgreSQL connections

Status: proposed · Release: 0.6.0 · Implements workspace ADR 0028
Interview outcome in one line: servers are a per-user registry in `servers.toml` with OS keychain passwords, `rok db` commands take `--server`, typed-name confirmation for drops, and `db query` runs in a transaction committing only on success.

## Problem Statement

Today every `rok db` command works against `DATABASE_URL` (the project's database). Users working with multiple PostgreSQL servers — local dev, a staging box, a managed cloud instance — must swap `DATABASE_URL` or pass `--database-url` every time. The connection string, including its password, often ends up in shell history or project-local `.env` files that get committed.

ADR 0028 decides:
- **Servers are per user, not per project.** A **Server** is a named PostgreSQL connection registered once (`rok server add local --url …`) in the user's config directory (`servers.toml`), its password in the OS keychain and never in a plain file.
- `rok db` commands that act on a server (creating, dropping, querying databases) take `--server`.
- A project's `DATABASE_URL` keeps working unchanged and stays the default for the commands that act on the project's own database.
- Where no keychain exists (headless Linux, CI), the password comes from `ROK_SERVER_<NAME>_PASSWORD`, and `server add` says so.
- Dropping a database asks for its name to be typed (`--yes` for scripts) and refuses the one the project's `DATABASE_URL` names unless `--force`.
- `db query` runs in a transaction that commits only on success.

The project registry and `rok new` are explicitly OUT of scope (ADR 0028 puts those in a separate workspace).

## Vocabulary

**Server** (GLOSSARY.md):
A named PostgreSQL server connection a user registers once and reaches from any project, to create, drop or query databases on it.
_Avoid_: connection (for the named, registered thing), profile, host

**Keychain** (this spec):
The OS credential store: macOS Keychain, Windows Credential Manager, Linux `libsecret` (via `secret-service`). When unavailable, the environment variable fallback `ROK_SERVER_<NAME>_PASSWORD` is used.

**Project database**:
The database identified by the project's `DATABASE_URL`. It is the default target for `rok db check`, `rok db inspect`, and `rok migrate …` when no `--server` is given.

## Decisions

| # | Decision | Record |
|---|----------|--------|
| D1 | Server definitions live in `<config-dir>/servers.toml`; passwords never touch the file | ADR 0028 |
| D2 | Passwords stored in OS keychain; `ROK_SERVER_<NAME>_PASSWORD` fallback for headless/CI | ADR 0028 |
| D3 | New top-level command group `rok server add|list|remove` | ADR 0028 |
| D4 | `rok db create|drop|query` accept `--server <name>` to target a registered server | ADR 0028 |
| D5 | `db drop` requires typed-name confirmation (`--yes` bypasses); refuses project's own `DATABASE_URL` database unless `--force` | ADR 0028 |
| D6 | `db query` runs in a transaction that commits only on success | ADR 0028 |
| D7 | `servers.toml` schema: `name`, `host`, `port`, `user`, `database` (optional default DB), `sslmode` (optional) | — |
| D8 | Config directory: `$XDG_CONFIG_HOME/rok` (Linux), `~/Library/Application Support/rok` (macOS), `%APPDATA%\rok` (Windows) | — |

## `servers.toml` schema

```toml
# ~/.config/rok/servers.toml  (or platform equivalent)

[[servers]]
name = "local"
host = "localhost"
port = 5432
user = "postgres"
database = "postgres"   # default database to connect to for admin ops
sslmode = "prefer"      # optional: disable|allow|prefer|require|verify-ca|verify-full

[[servers]]
name = "staging"
host = "staging-db.example.com"
port = 5432
user = "rok"
database = "postgres"
sslmode = "require"
```

The `database` field is the default database to connect to for administrative operations (create/drop/query other databases). It is not the project's database.

## Command surface

### `rok server add <NAME> --url <URL>` (also `--host`, `--port`, `--user`, `--database`, `--sslmode`)

Registers a server. The URL form parses `postgres://user:password@host:port/db?sslmode=...`. The password is extracted and stored in the OS keychain under service `rok`, account `<NAME>`. The remaining fields are written to `servers.toml`.

If no keychain is available, the command prints a warning and instructs the user to set `ROK_SERVER_<NAME>_PASSWORD` in the environment.

```
$ rok server add local --url postgres://postgres:secret@localhost:5432/postgres
Server 'local' added. Password stored in macOS Keychain (service: rok, account: local).

$ rok server add staging --host staging-db.example.com --user rok --password-prompt
# or: --url without password, then prompt
Server 'staging' added. Password stored in Windows Credential Manager.

$ rok server add ci --url postgres://user@host/db   # no password in URL
warning: no OS keychain available; set ROK_SERVER_CI_PASSWORD in the environment
Server 'ci' added (password from ROK_SERVER_CI_PASSWORD).
```

### `rok server list`

Lists registered servers, masking passwords. Shows the resolved host:port and whether a password is present in the keychain/env.

```
$ rok server list
NAME     HOST                    PORT  USER   DEFAULT DB  PASSWORD
local    localhost               5432  postgres  postgres   keychain ✓
staging  staging-db.example.com  5432  rok       postgres   keychain ✓
ci       ci-db.example.com       5432  ciuser    postgres   env ROK_SERVER_CI_PASSWORD ✓
```

### `rok server remove <NAME>`

Removes the server from `servers.toml` and deletes its password from the keychain (or instructs to unset the env var).

```
$ rok server remove ci
Server 'ci' removed. Unset ROK_SERVER_CI_PASSWORD if no longer needed.
```

### `rok db create <DBNAME> [--server <NAME>] [--database-url <URL>] [--json]`

Creates a database on the target server. `--server` selects a registered server; `--database-url` overrides with an ad-hoc connection (for one-offs). Without either, uses the project's `DATABASE_URL` (the project database) — but creating a database on the project's own server is unusual; the command will warn.

```
$ rok db create myapp_staging --server staging
Database 'myapp_staging' created on server 'staging'.

$ rok db create test_db --database-url postgres://user:pass@host/db
Database 'test_db' created.
```

### `rok db drop <DBNAME> [--server <NAME>] [--database-url <URL>] [--yes] [--force] [--json]`

Drops a database. Requires typed confirmation: the user must type the database name exactly. `--yes` bypasses for scripts. Refuses to drop the database named in the project's `DATABASE_URL` unless `--force` is given.

```
$ rok db drop old_db --server local
Type the database name to confirm: old_db
Database 'old_db' dropped.

$ rok db drop myapp_prod --server prod --yes
Database 'myapp_prod' dropped.

$ rok db drop myapp --server local   # myapp is the project's DATABASE_URL database
error: refusing to drop the project's own database (DATABASE_URL). Use --force to override.
```

### `rok db query <SQL> [--server <NAME>] [--database-url <URL>] [--json]`

Executes a SQL statement in a transaction that commits only on success. If the statement fails, the transaction rolls back. Works like `psql -c` but with the safety of a transaction. Multiple statements are allowed; they run in one transaction.

```
$ rok db query "CREATE TABLE t (id serial PRIMARY KEY); INSERT INTO t DEFAULT VALUES;" --server staging
OK: 2 statements executed, transaction committed.

$ rok db query "INSERT INTO t VALUES (1); SELECT 1/0;" --server local
error: division by zero
Transaction rolled back.
```

### `rok db check|inspect` — unchanged default behavior

These continue to use `DATABASE_URL` by default (the project database). They accept `--server` and `--database-url` for explicit targeting.

```
$ rok db check --server staging
ok: connected to staging-db.example.com:5432
pool: 1 of 1 connections
server: PostgreSQL 16.2 on x86_64-pc-linux-gnu, compiled by gcc...
```

## Keychain integration

Use the `keyring` crate (supports macOS Keychain, Windows Credential Manager, Linux `libsecret`). Service name: `rok`. Account name: the server name.

Fallback order when retrieving a password:
1. OS keychain (service `rok`, account `<NAME>`)
2. Environment variable `ROK_SERVER_<NAME>_PASSWORD` (uppercase, hyphens → underscores)
3. Error: "no password for server '<NAME>': not in keychain and ROK_SERVER_<NAME>_PASSWORD not set"

When adding a server, if the keychain write fails (no keychain available), warn and fall back to env var instruction.

## Config directory resolution

Use `dirs-next` crate:
- Linux: `$XDG_CONFIG_HOME/rok` or `~/.config/rok`
- macOS: `~/Library/Application Support/rok`
- Windows: `%APPDATA%\rok`

The `servers.toml` file lives there. Create the directory on first `server add`.

## `rok db` command changes

The existing `DbAction` enum gains new variants and flags:

```rust
enum DbAction {
    Check { database_url, server, json },
    Inspect { table, database_url, server, json },
    Create { name, database_url, server, json },
    Drop { name, database_url, server, yes, force, json },
    Query { sql, database_url, server, json },
}
```

Connection resolution precedence:
1. `--database-url` (explicit ad-hoc)
2. `--server <NAME>` (resolved via `servers.toml` + keychain/env)
3. `DATABASE_URL` (project default)

When `--server` is used, the connection URL is built from `servers.toml` entry + password from keychain/env. The `database` field from `servers.toml` is used as the initial connection database (typically `postgres` for admin ops).

## Transaction semantics for `db query`

- Begin transaction
- Execute all statements (split by `;` but respecting dollar quotes, strings, comments)
- On success: commit
- On error: rollback, print error, exit 1
- Output: row counts for DML, result sets for SELECT (tabular or JSON)

## Out of scope

- Project registry (ADR 0028: separate workspace)
- `rok new` (ADR 0028: separate workspace)
- Server groups, aliases, or inheritance
- SSH tunneling / bastion hosts
- Connection pooling configuration per server
- `db query` result formatting beyond tabular/JSON (no CSV, no vertical)

## Testing

- Unit tests for `servers.toml` parsing, URL building, keychain/env fallback logic
- Integration tests (require keychain or `ROK_SERVER_*_PASSWORD` set):
  - `server add/list/remove` round-trip
  - `db create/drop/query` against a test server
  - Typed-name confirmation for drop
  - Project database protection (`--force` override)
  - Transaction rollback on query error
- CI: uses `ROK_SERVER_TEST_PASSWORD` for a PostgreSQL service container

## Tickets

See `.scratch/servers/issues/` for tracer-bullet tickets.