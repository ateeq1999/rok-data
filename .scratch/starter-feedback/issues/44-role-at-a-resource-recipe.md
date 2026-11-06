# 44: Guards: authorization rules written once

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the templates' security Capability gains:
- **A trait:** `trait Guard { async fn allows(caller: &AuthUser, state: &AppState, path: &Path) -> Result<bool, AppError>; }`.
- **An extractor:** `Allowed<G: Guard>`, which answers 403 when the guard refuses. It is a caller extractor, so ticket 52's rule keeps it ahead of the body.
- **A recipe:** the guide shows three guards in Karibu Pass's shape: a staff role (`Finance`), another staff role (`Partnerships`), and a member's role at a resource from the path (`PartnerMember`). Each is backed by one shared query (ticket 42), so a project writes each rule once and handlers take `Allowed<Finance>`.

The glossary's **Guard** names it.

**Blocked by:** 42, grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] `Allowed<G>` refuses with 403 before the body is read, and passes the caller through when the guard allows (tests)
- [ ] The recipe's three guards compile in a gate or a doc test
- [ ] The Shared files stay identical across templates
