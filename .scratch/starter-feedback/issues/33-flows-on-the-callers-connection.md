# 33: Cross-feature flows run on the caller's connection

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in the templates, the flows other features compose get `_in(conn, …)` variants that run on the caller's connection: `invite_in`, `register_in` and the audit log's `record_in`. The existing methods open their own transaction and call the `_in` variant. The template's `AGENTS.md` states the convention: a flow other features may compose gets an `_in` variant.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** ready-for-agent

- [ ] Another feature can invite and record an audit entry inside its own transaction, and a rollback undoes both (test)
- [ ] Existing callers unchanged
