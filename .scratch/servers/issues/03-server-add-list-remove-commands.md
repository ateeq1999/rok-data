# Ticket 03: `rok server add|list|remove` commands

**Blocked by:** 01-servers-toml-and-config-dir, 02-keychain-integration
**Status:** done

## Scope

- New CLI module `crates/rok-cli/src/commands/server.rs`
- New `ServerAction` enum in `cli.rs`: `Add`, `List`, `Remove`
- Wire into `Command` enum and `execute()` in `commands/mod.rs`

### `rok server add <NAME> --url <URL>` (primary)
- Parse `postgres://user:password@host:port/db?sslmode=...`
- Extract password → keychain
- Write remaining fields to `servers.toml`
- Also accept discrete flags: `--host`, `--port`, `--user`, `--database`, `--sslmode`, `--password-prompt`
- Validate: name unique, URL or discrete flags provided (not both), password from URL or prompt

### `rok server list`
- Read `servers.toml`
- For each server, check keychain/env for password presence
- Output table: NAME, HOST, PORT, USER, DEFAULT DB, PASSWORD (keychain ✓ / env VAR ✓ / missing ✗)

### `rok server remove <NAME>`
- Remove from `servers.toml`
- Delete password from keychain (or instruct to unset env var)
- Confirm before removing (unless `--yes`)

## Acceptance criteria

- `rok server add local --url postgres://postgres:pass@localhost:5432/postgres` → server added, password in keychain
- `rok server list` shows formatted table with password status
- `rok server remove local` → server gone, password deleted
- Duplicate name errors; unknown name errors on remove
- `--json` flag on all three for machine output

## Files to touch

- `crates/rok-cli/src/cli.rs` — add `ServerAction` enum, add `Server(ServerAction)` to `Command`
- `crates/rok-cli/src/commands/server.rs` — new module with `add`, `list`, `remove`
- `crates/rok-cli/src/commands/mod.rs` — wire `Command::Server(action)` in `execute()`
- `crates/rok-cli/src/output.rs` — add JSON serialization for server list

## Notes

- Follow existing patterns: `DbAction`, `MigrateAction`, `SchemaAction`
- URL parsing: use `url` crate (already in workspace via sqlx)
- Password prompting: use `rpassword` crate or `dialoguer`
- `sslmode` values: `disable|allow|prefer|require|verify-ca|verify-full` (PostgreSQL standard)