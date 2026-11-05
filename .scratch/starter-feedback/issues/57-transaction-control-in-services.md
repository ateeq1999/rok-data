# 57: Transaction control in services

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:**
- **The rule:** `check-arch` rule 2 still forbids SQL in a service, but allows transaction control: a `Transaction`, and `begin` (a savepoint inside one), `commit` and `rollback`.
- **Template half (after the gRPC chain):** the template's `AGENTS.md` documents the `ON CONFLICT … DO NOTHING` + `:execrows` retry as the simpler choice when it fits, and the templates enable `reqwest`'s `json` feature.

**Blocked by:** None for the rule; grpc-api-template/08 for the template half

**Status:** ready-for-agent

- [ ] A service using a savepoint passes check-arch; one calling `sqlx::query` still fails
- [ ] Template: AGENTS.md pattern, `reqwest` `json` enabled
