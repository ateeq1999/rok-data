# Ticket 04: `rok db create|drop` with --server support

**Blocked by:** 01-servers-toml-and-config-dir, 02-keychain-integration
**Status:** ready-for-agent

## Scope

- Extend `DbAction` in `cli.rs` with `Create` and `Drop` variants
- Add `--server <NAME>` flag to both (mutually exclusive with `--database-url`)
- Implement connection resolution: `--database-url` > `--server` > `DATABASE_URL`
- When `--server` used: build URL from `servers.toml` entry + password from keychain/env
- The `database` field from `ServerConfig` is the initial connection database (typically `postgres`)

### `rok db create <DBNAME> [--server <NAME>] [--database-url <URL>] [--json]`
- Connect to target server (using admin database from server config)
- Execute `CREATE DATABASE <DBNAME>`
- Output: "Database '<DBNAME>' created on server '<NAME>'." or JSON

### `rok db drop <DBNAME> [--server <NAME>] [--database-url <URL>] [--yes] [--force] [--json]`
- **Typed-name confirmation**: prompt "Type the database name to confirm: " — must match exactly
- `--yes` bypasses prompt (for scripts)
- **Project database protection**: if target DB name matches the database in `DATABASE_URL` (project's own DB), refuse unless `--force`
- Execute `DROP DATABASE <DBNAME>`
- Output: "Database '<DBNAME>' dropped." or JSON

## Acceptance criteria

- `rok db create mydb --server staging` creates database on staging server
- `rok db drop olddb --server local` prompts for name, drops on match
- `rok db drop myapp --server local` (where myapp is project's DATABASE_URL) refuses without `--force`
- `--yes` skips prompt; `--force` allows dropping project DB
- `--json` outputs structured result
- Connection resolution precedence works correctly

## Files to touch

- `crates/rok-cli/src/cli.rs` — add `Create`, `Drop` to `DbAction` with flags
- `crates/rok-cli/src/commands/db.rs` — add `create()`, `drop()` functions
- `crates/rok-cli/src/context.rs` — add `resolve_with_server(server_name)` to build connection from server config
- `crates/rok-cli/src/commands/mod.rs` — wire new `DbAction` variants in `execute()`

## Notes

- Reuse `Connection::resolve()` pattern; add `resolve_server(server_name)` 
- For drop confirmation: read from stdin, trim, compare exactly
- Project DB detection: parse `DATABASE_URL`, extract database name, compare case-insensitively
- `CREATE DATABASE` / `DROP DATABASE` need autocommit (run outside transaction)