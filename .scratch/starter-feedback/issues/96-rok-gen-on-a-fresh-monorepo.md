# 96: `rok gen` runs on a freshly scaffolded Monorepo

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What's wrong:** `rok gen` fails on a Monorepo scaffolded from rok-templates 0.7.0, with or without `-s`. The error says two schemas are named `TokenResponse`:
- `src/features/auth/dto.rs`
- `src/features/two_factor/dto.rs`

rok-templates `spec/starter-feedback` now has a third one, in `src/features/oidc/dto.rs` (ticket 88). The same three exist in the axum and actix templates.

Ticket 03 found this. Its test shows that `rok gen` reads the seed, but it can't show a successful run.

**What to build** (decided 2026-11-24: fix it in the templates, not the engine):
- Rename the DTOs in rok-templates so every schema name is unique: auth keeps `TokenResponse`, and the others become for example `TwoFactorTokenResponse` and `OidcTokenResponse`.
- Do this in every template that has these features (axum, actix, the Monorepo API), then regenerate the contract and the Clients.
- Then ticket 03's test in rok-data (`rok_gen_reads_the_monorepo_seed`) should do a successful `rok gen` on the seeded Monorepo, once rok-data takes a rok-templates release with the rename.

**Blocked by:** None (can start immediately)

**Status:** blocked (the rename is on rok-templates `spec/starter-feedback`, PR #19; the seed-test box waits on rok-data taking a rok-templates release with it)

- [x] `rok gen` succeeds on a freshly scaffolded Monorepo (test)
- [ ] Ticket 03's seed test does a successful `rok gen` run
