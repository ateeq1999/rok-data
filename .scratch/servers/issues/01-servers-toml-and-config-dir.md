# Ticket 01: servers.toml and config directory

**Blocked by:** none
**Status:** ready-for-agent

## Scope

- Resolve the per-user config directory (`dirs-next`: Linux `$XDG_CONFIG_HOME/rok`, macOS `~/Library/Application Support/rok`, Windows `%APPDATA%\rok`)
- Create the directory on first use
- Define `servers.toml` schema and parsing (serde, deny_unknown_fields)
- Read/write `servers.toml` with atomic writes (write to temp, rename)
- Types: `ServerConfig { name, host, port, user, database, sslmode }`

## Acceptance criteria

- `config_dir()` returns the correct platform path
- `servers.toml` is created with correct permissions (0600)
- Round-trip: write → read yields identical data
- Unknown keys in `servers.toml` are an error
- Missing file is not an error (empty server list)

## Files to touch

- `crates/rok-cli/src/config.rs` — add `ServersConfig`, `ServerConfig`, `servers_path()`, `read_servers()`, `write_servers()`
- `crates/rok-cli/src/commands/server.rs` — new module for `rok server` commands (created in ticket 02)

## Notes

- Follow the existing config.rs patterns: `ConfigFile`, `parse()`, `FILE`, `LEGACY_FILE`
- No keychain yet — this ticket is only the TOML file
- The `database` field in `ServerConfig` is the default DB to connect to for admin ops (typically `postgres`)