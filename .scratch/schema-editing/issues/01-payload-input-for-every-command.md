# 01: Every command takes a JSON payload

Spec: `docs/specs/schema-editing.md`

**What to build:** in rok-data-cli, every command accepts `-j`/`--payload` with an inline JSON object, a file path, `@path` or `-` (stdin); a value starting with `{` or `[` is JSON, anything else a path. The object's keys are the command's long flag names (positionals by value name). Command-line flags override payload keys; an unknown key is a usage error naming it. `--json` stays the output switch. Help text, the CLI reference and the command examples show it.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] The four forms work for one command (tests through the CLI)
- [ ] Override and unknown-key behaviour tested
- [ ] Every command group accepts it (one test per group)
- [ ] `--json` output unchanged everywhere
