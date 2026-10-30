# Ticket 06: Update docs/cli-reference.md for servers and new db commands

**Blocked by:** 03-server-add-list-remove-commands, 04-db-create-drop-with-server, 05-db-query-transaction
**Status:** ready-for-agent

## Scope

- Add `rok server` section to `docs/cli-reference.md`
- Update `rok db` section with `create`, `drop`, `query` subcommands
- Document `--server` flag behavior and precedence
- Document typed-name confirmation for `drop`
- Document project database protection (`--force`)
- Document transaction semantics for `query`
- Update the root command summary at top of file

## Acceptance criteria

- `docs/cli-reference.md` matches the implemented CLI grammar exactly
- All flags, output formats, exit codes documented
- Examples match actual command behavior
- Machine-readable `--json` output shapes documented

## Files to touch

- `docs/cli-reference.md` — update per the contract (ADR 0012: CLI reference is contract)

## Notes

- This must be done in the same PR as the grammar changes (workspace invariant)
- Follow existing format: tables for flags, usage blocks, examples
- The `sqlc` subtree is documented separately (engine's grammar); this is only the `rok` CLI