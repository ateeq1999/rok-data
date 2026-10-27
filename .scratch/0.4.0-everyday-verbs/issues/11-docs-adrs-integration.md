# 11: Documentation, ADRs, and vertical slice integration

**What to build:** All user-facing docs updated for 0.4.0 features; ADRs recorded; `examples/api` uses the new surface.

**Blocked by:** 01–10 (all features must be complete for docs to be accurate).

**Status:** done (2026-10-27) — two items deferred by decision, below

- [x] `docs/getting-started.md` — an everyday-verbs section (typed conditions, relation filters, counts, nested loading, row verbs) with its sample compiled from `crates/rok-data/tests/docs/getting_started.rs`; the rest of the surface is documented on the items themselves (rustdoc) and in the CHANGELOG
- [x] `docs/feature-matrix.md` — rows for relation filters and aggregates, through and many-to-many, nested loading, collections, encrypted columns and value objects
- [x] `docs/unified-pipeline-plan.md` — Phase 7 entry
- [ ] Root `CHANGELOG.md` `## [0.4.0]` section — **deferred by decision:** the work is recorded in the unreleased section at the top (headed 0.3.0, the workspace still at 0.2.0); which release carries it, and its heading, is the maintainer's call when the release is cut
- [x] Workspace ADR 0017 — collections and loaded views
- [x] Workspace ADR 0018 — relations describe their own batch
- [x] Workspace ADR 0019 — a field's type is its cast (replaces the casting-system ADR)
- [x] Workspace ADR 0020 — typed chain sugar and row verbs (pagination shapes were settled by dx-first 09–16 and `docs/pagination.md`)
- [ ] `examples/api` using the new surface — **deferred by decision:** its gate passes unchanged (`rok gen check`, compile, DB tests); it has no join table, through relation or encrypted column to show, and adding them only to exercise the verbs would change its schema for no reader's benefit
- [x] CI: `cargo fmt --all`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace --all-targets` green
