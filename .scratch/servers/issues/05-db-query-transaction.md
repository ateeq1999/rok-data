# Ticket 05: `rok db query` with transaction commit-on-success

**Blocked by:** 01-servers-toml-and-config-dir, 02-keychain-integration
**Status:** ready-for-agent

## Scope

- Add `Query` variant to `DbAction` in `cli.rs`
- Accept SQL as positional argument (string, can contain semicolons)
- `--server` and `--database-url` flags (mutually exclusive)
- `--json` flag for machine output

### Transaction semantics
- Begin transaction
- Split SQL by `;` respecting:
  - Dollar-quoted strings (`$tag$...$tag$`)
  - Single-quoted strings (`'...'`)
  - Double-quoted identifiers (`"..."`)
  - Line comments (`-- ...`)
  - Block comments (`/* ... */`)
- Execute each statement in the same transaction
- On all success: commit
- On any error: rollback, print error, exit 1

### Output
- Default: tabular for SELECT, "OK: N statements executed, transaction committed." for others
- `--json`: `{"statements": N, "results": [...], "committed": true}` or error object

## Acceptance criteria

- `rok db query "CREATE TABLE t (id serial); INSERT INTO t DEFAULT VALUES;" --server local` commits both
- `rok db query "INSERT INTO t VALUES (1); SELECT 1/0;" --server local` rolls back on division by zero
- Multiple SELECTs return multiple result sets
- `--json` produces valid JSON with results or error
- Statements split correctly with dollar quotes, strings, comments

## Files to touch

- `crates/rok-cli/src/cli.rs` — add `Query` to `DbAction`
- `crates/rok-cli/src/commands/db.rs` — add `query(sql, server, database_url, json)`
- `crates/rok-cli/src/commands/mod.rs` — wire `DbAction::Query`

## Notes

- SQL splitting: use a proper parser or careful state machine; don't naively split on `;`
- `sqlx::query()` for execution; `sqlx::query_as()` for SELECT results
- For result sets: detect if statement returns rows (check `rows_affected` vs `fetch`)
- Transaction: use `db.begin().await?`, then `tx.execute()` / `tx.fetch_all()`, then `tx.commit().await?`
- Error handling: rollback on any error, return error message