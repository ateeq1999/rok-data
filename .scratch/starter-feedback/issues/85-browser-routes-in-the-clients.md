# 85: Browser routes stay in the contract, and each Client builds their URL

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it) · engine ADR 0027

**What to build:** an operation whose every response is a `3xx` is a **Browser route** (engine glossary). The contract reader accepts it instead of failing with "the operation has no success (`2xx`) response", and each Client gets a URL builder in place of a call: it takes the path and query parameters and returns the full URL against the Client's base URL (Dart `oidcStartUrl(...)`, TS `oidcStartUrl(...)`, Rust `oidc_start_url(...)`). An operation with any `2xx` is unchanged. No `x-rok-client` extension.

Karibu Pass left `GET /auth/oidc/start` and `GET /auth/oidc/callback` out of `openapi.rs` so `rok gen` would run, and nothing checks them now (ticket 26).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A `302`-only operation reads as a Browser route; one with no `2xx` and no `3xx` still fails (reader tests)
- [ ] Each Client emits a URL builder with path and query parameters, and no call (emission tests)
- [ ] Engine ADR 0027's Client sections name Browser routes
