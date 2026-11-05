# 44: Recipe: a caller's role at a resource

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** a template guide recipe for a path-scoped role extractor (for example a member's role at an organisation from `/orgs/{id}/…`), built like `AuthUser`/`AdminUser`, with one shared query (ticket 42) and one `403` message, so features don't each repeat the check.

**Blocked by:** 42, grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] The recipe's code compiles in a gate or a doc test
