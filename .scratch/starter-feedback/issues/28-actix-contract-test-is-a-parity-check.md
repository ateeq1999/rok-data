# 28: The actix contract test says it's a parity check

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in the actix-api template's `tests/contract.rs`, a `KNOWN_SCHEMAS` list marks a whole schema (`UserResponse`) as the project's own, so the comparison skips it in one line instead of one `KNOWN` entry per difference. The README says the test keeps parity with the axum starter and is expected to grow `KNOWN` / `KNOWN_SCHEMAS` entries as the project changes its own schema; its stale "Coming next" section is fixed.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** ready-for-agent

- [ ] A schema in `KNOWN_SCHEMAS` is skipped; one no longer differing fails as stale, like `KNOWN`
- [ ] README updated
