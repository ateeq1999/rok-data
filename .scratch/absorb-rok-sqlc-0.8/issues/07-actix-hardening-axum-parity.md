# 07: Actix hardening and axum parity (rok-sqlc 0.7)

**What to build:** The axum template gains the account flows (moved here from ticket 05, ported from upstream v0.7.0's `templates/axum-api`). The actix template hashes passwords on a blocking thread, its rate limit lives where upstream moved it, and its `/api/v1/auth` scope answers unknown paths and methods with the wire standard. Under axum, a malformed path or query answers `400 bad_request` with the wire wording. A repository test fails when a framework-independent template file differs between the two templates.

**Blocked by:** 05

**Status:** ready-for-agent

- [ ] Ported behavior in both templates where it applies
- [ ] Template-drift test in rok-cli's suites
- [ ] Scaffolded `AGENTS.md` / `features/README.md` worded for `rok sqlc` and `rok.toml`
