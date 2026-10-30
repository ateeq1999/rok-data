# Ticket 07: Integration tests for servers and db commands

**Blocked by:** 03-server-add-list-remove-commands, 04-db-create-drop-with-server, 05-db-query-transaction
**Status:** done

## Scope

- Add integration tests in `crates/rok-cli/tests/` (following existing pattern: spawn `CARGO_BIN_EXE_rok`)
- Tests require a PostgreSQL server; use `DATABASE_URL` or `ROK_SERVER_TEST_PASSWORD` for CI
- Test categories:

### Server registry tests
- `server_add_list_remove_roundtrip`: add → list → remove → list empty
- `server_add_duplicate_error`: add same name twice → error
- `server_remove_unknown_error`: remove non-existent → error
- `server_list_json`: `--json` output structure
- `server_keychain_fallback`: with `ROK_SERVER_<NAME>_PASSWORD` set, no keychain

### db create/drop tests
- `db_create_on_server`: create DB on registered server, verify exists
- `db_drop_on_server`: create then drop, verify gone
- `db_drop_typed_confirmation`: drop prompts, type name, succeeds
- `db_drop_yes_flag`: `--yes` skips prompt
- `db_drop_project_db_refused`: drop project's DATABASE_URL DB → refused
- `db_drop_project_db_force`: `--force` allows dropping project DB
- `db_create_drop_json`: `--json` output structure

### db query tests
- `db_query_commit`: multiple statements, all succeed → committed
- `db_query_rollback`: statement fails → rolled back, error
- `db_query_select`: SELECT returns results (tabular and JSON)
- `db_query_transaction_isolation`: failed statement doesn't affect committed state

### Connection resolution tests
- `db_precedence_database_url`: `--database-url` wins over `--server`
- `db_precedence_server`: `--server` wins over `DATABASE_URL`
- `db_precedence_env`: neither flag → `DATABASE_URL` used

## Acceptance criteria

- All tests pass in CI (with PostgreSQL service container)
- Tests are honest: print skip message when DB not available (like existing migrate tests)
- Tests clean up after themselves (drop created databases)
- No flaky tests; deterministic assertions

## Files to touch

- `crates/rok-cli/tests/servers.rs` — new test file
- `crates/rok-cli/tests/db_commands.rs` — new test file (or extend existing)

## Notes

- Follow existing test pattern: `tokio::test`, `Command::new(env!("CARGO_BIN_EXE_rok"))`
- Use unique database names per test (include test name + random suffix)
- Set `ROK_SERVER_TEST_PASSWORD` in CI for the test server
- The existing `migrate` tests in `cli.rs` are the model