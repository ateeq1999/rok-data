# 05: The gRPC auth interceptor

Spec: `docs/specs/grpc-api-template.md` · workspace ADR 0030 ("gRPC verifies auth, it does not issue it")

**What to build:** the `grpc-api` template's interceptor accepts the bearer tokens (`axum-api`'s JWT: same secret, same claims) and API keys `axum-api` issues, and puts the caller where the service trait reads it, as the HTTP extractor does. No credentials or bad ones → `UNAUTHENTICATED`. No auth RPCs.

**Blocked by:** 04

**Status:** blocked

- [ ] Valid JWT → the call proceeds with that caller
- [ ] Valid API key → the call proceeds
- [ ] Missing, expired or forged token → `UNAUTHENTICATED`
- [ ] Health and reflection stay reachable without credentials
