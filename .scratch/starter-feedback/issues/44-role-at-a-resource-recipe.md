# 44: Guards: authorization rules written once

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the templates' security Capability gains:
- **A trait:** `trait Guard { type Grant; async fn check(caller: &AuthUser, state: &AppState, path: &Path) -> Result<Option<Self::Grant>, AppError>; }`. The grant says how access was given (a role, or "staff"); a plain yes/no guard uses `Grant = ()`.
- **An extractor:** `Allowed<G: Guard>`, which answers 403 on `None` and hands the handler the grant (`Allowed(access): Allowed<PartnerAccess>`). It is a caller extractor, so ticket 52's rule keeps it ahead of the body.
- **A recipe:** the guide shows guards in Karibu Pass's shape: a staff role (`Finance`, `Grant = ()`), and access to a resource from the path that is either a member's role or staff (`PartnerAccess` with `Grant = enum { Member(PartnerRole), Staff(StaffRole) }`). Each is backed by one shared query (ticket 42), so a project writes each rule once. The recipe also shows a guard generic over a marker type, `Allowed<PartnerAccess<Organizer>>`, whose one query checks membership and the Partner's kind together and answers 403 for the wrong kind ("this isn't an organizer"), so features that serve one kind of Partner share it. The kinds are the project's; the template ships only `Guard` and `Allowed`.

The glossary's **Guard** names it.

**Blocked by:** 42, grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] `Allowed<G>` refuses with 403 before the body is read, and hands the grant through when the guard allows (tests)
- [ ] The recipe's three guards compile in a gate or a doc test
- [ ] The Shared files stay identical across templates
