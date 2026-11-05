# 24: One rule for finding the project

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:**
- **Search:** `rok gen`, `rok gen check`, `rok migrate *`, `rok schema *`, `rok db *` and `rok sqlc check-arch` walk up from the current directory to the nearest `rok.toml`.
- **Inside an App:** started inside an App named in `[apps]`, a command acts on that App.
- **At the root:** a command acts on the App(s) `[apps]` names; `rok migrate up` takes the Apps' migrations directories. An explicit `--dir` still wins.
- **Report:** each command prints one line naming the config and directories it read (stderr, so `--json` stdout stays clean).
- **Repro:** ask Karibu Pass for their `apps/api/rok-data-sqlc.toml` (item 4 doesn't reproduce from the code).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `rok migrate up` at a Monorepo root migrates the api App
- [ ] Each command, run at the root and inside an App, reads the same App and says so
- [ ] `rok sqlc check-arch` finds a `rok.toml` project
