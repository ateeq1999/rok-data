# 57: Transaction control in services

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:**
- **The rule:** `check-arch` rule 2 still forbids SQL in a service, but allows transaction control: a `Transaction` or `rok_data::Tx`, `begin` (a savepoint inside one), `commit`, `rollback`, and the closure helper below.
- **A closure helper on a pool:** the runtime has `Db::transaction(|tx| Box::pin(async move { … }))`; add `rok_data::transaction(&pool, |tx| …)` for a plain `PgPool`, which is what template services hold, committing on `Ok` and rolling back on `Err`. Karibu wrote `begin` … `commit` by hand eleven times in one feature.
- **Template half (after the gRPC chain):** the template's `AGENTS.md` documents the `ON CONFLICT … DO NOTHING` + `:execrows` retry as the simpler choice when it fits, and the templates enable `reqwest`'s `json` feature.

**Blocked by:** None for the rule; grpc-api-template/08 for the template half

**Status:** ready-for-agent

- [ ] A service using a savepoint, or `rok_data::transaction`, passes check-arch; one calling `sqlx::query` still fails
- [ ] `rok_data::transaction(&pool, …)` commits on `Ok`, rolls back on `Err` (DB-backed test)
- [ ] Template: `AGENTS.md` shows it for an audited multi-step write
- [ ] Template: AGENTS.md pattern, `reqwest` `json` enabled
