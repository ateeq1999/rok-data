# 11: Documentation, ADRs, and vertical slice integration

**What to build:** All user-facing docs updated for 0.4.0 features; ADRs recorded; `examples/api` uses the new surface.

**Blocked by:** 01–10 (all features must be complete for docs to be accurate).

**Status:** ready-for-agent

- [ ] `docs/getting-started.md` — new sections: collections, query sugar, relationships (through/many-to-many/aggregates), scopes (params/global), casting, pagination, model lifecycle, eager loading, serialization
- [ ] `docs/feature-matrix.md` — new rows for each capability
- [ ] `docs/unified-pipeline-plan.md` — 0.4.0 phase entry with milestone checkboxes
- [ ] Root `CHANGELOG.md` — `## [0.4.0]` section
- [ ] **ADR 0016** — ModelCollection and collection methods
- [ ] **ADR 0017** — Relationship extensions (through, many-to-many, aggregates, existence)
- [ ] **ADR 0018** — Casting system (standard, custom, value objects, inbound, castables)
- [ ] **ADR 0019** — Pagination shapes (simple_paginate, Laravel JSON, cursor prev, OpenAPI)
- [ ] `examples/api` — regenerate with all 0.4.0 features; verify `rok gen check` passes
- [ ] CI: `cargo fmt --all`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace --all-targets` green