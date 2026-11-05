# 40: Actix features mount resources, not scopes

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the engine's actix routes for a generated feature use `web::resource` only, never `web::scope`, so a feature can't answer 404 for another feature's paths under a shared prefix. `check-arch` warns when a hand-written feature's `web::scope` prefix covers another feature's routes.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] Two features under one prefix (`/partner` and `/partner/{id}/offers`) both answer, in either mount order (test)
- [ ] check-arch warns on a covering scope
