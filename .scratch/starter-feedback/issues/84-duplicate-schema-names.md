# 84: Two schemas with one name fail `rok gen`

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** utoipa keeps one schema when two `ToSchema` types share a component name, so a hand-written DTO can silently replace a generated one and the Clients come out wrong. `rok gen` and `rok gen check` scan the App's source for `ToSchema` types (with any `#[schema(as = …)]` rename) and fail when two share a component name. The error names both files, and says to rename one or give it `#[schema(as = …)]`.

Karibu Pass's hand-written `OfferDropResponse` replaced the generated one, and the admin list's Rust Client got a `PaginatedResponseOfferDropResponseItemsItem` item type (ticket 25).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A hand-written DTO named like a generated response fails `rok gen`, naming both files (test)
- [ ] `#[schema(as = …)]` resolves the clash
