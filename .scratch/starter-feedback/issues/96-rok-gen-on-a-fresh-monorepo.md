# 96: `rok gen` runs on a freshly scaffolded Monorepo

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What's wrong:** `rok gen` fails on a Monorepo scaffolded from rok-templates 0.7.0, with or without `-s`. The error says two schemas are named `TokenResponse`:
- `src/features/auth/dto.rs`
- `src/features/two_factor/dto.rs`

rok-templates `spec/starter-feedback` now has a third one, in `src/features/oidc/dto.rs` (ticket 88). The same three exist in the axum and actix templates.

Ticket 03 found this. Its test shows that `rok gen` reads the seed, but it can't show a successful run.

**What to decide, then build:** one of these.
- **Templates:** rename the DTOs so each schema name is unique (for example `TwoFactorTokenResponse`, `OidcTokenResponse`), then regenerate the contract and the Clients.
- **Engine:** handle same-named schemas from different features, for example by qualifying them with the feature name or by refusing with a clearer message.

Whichever is chosen, ticket 03's test should then show a successful `rok gen` on the seeded Monorepo.

**Blocked by:** None (can start immediately)

**Status:** needs-triage

- [ ] `rok gen` succeeds on a freshly scaffolded Monorepo (test)
- [ ] Ticket 03's seed test does a successful `rok gen` run
