# Ticket 02: OS keychain integration (password storage)

**Blocked by:** 01-servers-toml-and-config-dir
**Status:** ready-for-agent

## Scope

- Add `keyring` crate dependency to `rok-cli`
- Implement `store_password(server_name, password)`, `get_password(server_name)`, `delete_password(server_name)`
- Service name: `rok`, account name: `<server_name>`
- Fallback: environment variable `ROK_SERVER_<NAME>_PASSWORD` (uppercase, hyphens → underscores)
- On keychain write failure (no keychain available): warn and instruct user to set env var
- On keychain read failure: try env var, then error with clear message

## Acceptance criteria

- `store_password` writes to keychain on macOS/Windows/Linux (with libsecret)
- `get_password` reads from keychain, falls back to `ROK_SERVER_<NAME>_PASSWORD`
- `delete_password` removes from keychain
- Clear error when neither keychain nor env var has the password
- Warning on `server add` when keychain unavailable

## Files to touch

- `crates/rok-cli/Cargo.toml` — add `keyring` dependency
- `crates/rok-cli/src/keychain.rs` — new module: `store`, `get`, `delete`, `KeychainError`
- `crates/rok-cli/src/commands/server.rs` — use keychain in `server add/remove`

## Notes

- `keyring` crate handles platform differences; test on each platform
- Env var name: `ROK_SERVER_LOCAL_PASSWORD` for server `local`, `ROK_SERVER_STAGING_DB_PASSWORD` for `staging-db`
- This is a hard dependency for the CLI; the keyring crate is well-maintained