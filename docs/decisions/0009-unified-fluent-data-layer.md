# ADR 0009: one fluent data layer for all generated code

Date: 2026-10-19 · Status: accepted

## Context

ADR 0006 kept two generator engines precisely because their jobs differ —
but that also forces users to pick a stylistic lane at project start:
whole-crate `rok generate` output is the fluent ORM (`#[derive(Model)]`
structs, typed column constants like `User::AGE`, chainable
`Select`/`Update`/`Insert` over `rok-data-core`), while
`rok sqlc generate` emits plain sqlx function modules that cannot touch
the ORM. The two idioms neither interoperate nor converge, and the
product direction — write schema SQL, get a running API, with fluent,
readable call sites, unified — demands one answer.

## Decision

All generated **data-layer** code becomes the fluent ORM style:
`Model` structs with typed column constants and chainable queries built
on `rok-data-core`, relationships as derive-generated relation constants.
The sqlc engine keeps its user-facing surface — resource module layout,
names, create/update patch semantics, the `XFilter`/sort/JSON query
contracts, API features — but its generated internals are rewritten to
compile down to the ORM instead of raw sqlx. Hand-written
`#[derive(Model)]` structs stay equally first-class; the facade story is
unchanged.

Considered and rejected: making plain sqlx function modules the single
style (fails the fluent/unified goal), and keeping both styles
permanently (the lane-choice problem stands).

## Consequences

- Generated data layers depend on the facade (`rok-data`), not raw sqlx.
- Feature parity gates the flip: struct filters, partial update patches,
  keyset/search matrix, eager loaders and search columns must all be
  reachable through ORM-backed implementations first.
- This is a breaking change to the shape of generated output; it ships
  with the rebuilt pipeline (ADR 0012) at the next minor release, and
  the legacy path keeps today's style exactly as is, per ADR 0005's
  marker policy.
