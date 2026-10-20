# ADR 0012: one front door — `rok init` + `rok gen`, axum-first templates

Date: 2026-10-19 · Status: accepted

## Context

The shortest path to a running API today is
`rok sqlc init --template actix`, then hand-writing
`src/{main,lib,app,config,error}.rs` plus auth and OpenAPI wiring before
`cargo run` — and the command surface splits the product (`rok generate`
versus `rok sqlc g`) along with the web-framework split (API features
are actix-only). The product goal is schema-in, running-API-out.

## Decision

- Rebuilt verbs own the pipeline: `rok init [NAME]` scaffolds a
  **Template** — a complete application crate including
  `main`/`app`/`config`/`error`/OpenAPI wiring — while
  `src/shared/auth.rs` and each feature's `service.rs` stay Scaffolded
  files (written once, user-owned, never rewritten).
  `rok gen` regenerates from the schema dir: ORM models (ADR 0009),
  migration pairs (ADR 0010), and API features.

- `rok generate [check]` and `rok sqlc` continue to work as legacy
  aliases: `rok sqlc` tokens are still forwarded verbatim to
  `rok_data_sqlc::cli::run_from` (upholding ADR 0002's invariant). The
  root help marks them as superseded; removal is deferred to 1.0.

- The unified Template targets **axum** first, with utoipa OpenAPI
  documents; actix-web users stay on the engine's legacy path.

Rejected: a runtime generic API server (`rok serve`, PostgREST-style) —
deferred. Honest Rust requires codegen for typed endpoints, and a second
mapping engine would double the truth surface.

## Consequences

- ADR 0002's command list gains `init`/`gen`; `docs/cli-reference.md`
  is contract and is updated in the same change as any grammar change.
- The engine gains an axum feature generator held to the same
  pedantic-lint compile bar as the actix fixtures.
- `rok gen` writes in-project outputs only (`src/db/**`,
  `src/features/**`); standalone-crate generation stays on the legacy
  `rok generate` path, with a future `[generate] target = "crate"` the
  planned way to widen it.
- Deliberate post-slice deferrals, served by the legacy paths until
  their own milestones: annotated SQL query files (`-- name: … :one`),
  derive-level many-to-many relations, and generated auth
  (`rok init --auth jwt`).
