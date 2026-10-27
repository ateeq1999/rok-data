# 11: Freeze rok-sqlc and close out the docs

**What to build:** One engine from here on. rok-sqlc's README says it is frozen at 0.8.0 and points to `rok sqlc`, with a final CHANGELOG entry. In this workspace: `docs/migration-audit.md` says rok-sqlc is absorbed through 0.8.0; the engine CHANGELOG records each absorbed upstream version; the upstream guide pages (auth, files, validation) are ported; the CHANGELOG's breaking-changes list names every new env var, migration and wire answer.

**Blocked by:** 01–10, dx-first 14

**Status:** done (the rok-sqlc commit is local, not pushed)

- [x] rok-sqlc README banner + final CHANGELOG entry (no code change, no release)
- [x] migration-audit, engine CHANGELOG, guides, AGENTS.md counts
- [x] `docs/cli-reference.md` matches the engine's tests
