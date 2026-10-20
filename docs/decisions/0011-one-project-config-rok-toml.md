# ADR 0011: one project config, `rok.toml`

Date: 2026-10-19 · Status: accepted

## Context

ADR 0006's "non-competing config" — `rok-data.toml` (`[gen]`) versus
`rok-data-sqlc.toml` (`[api]`, `[conventions]`, `[modules]`) — becomes
the main friction in a pipeline that must generate models, migrations
and API features in one pass. Shared notions (schema input, output
roots, conventions/preset) already exist in both files with different
homes, and the engine's per-key disable convention (empty string turns
a key off) needs one home too.

## Decision

One project-level `rok.toml` holds the pipeline configuration:
`[project]`, `[generate]`, `[api]`, `[conventions]` (the engine's `rok`
preset as default: tenant column, soft delete, managed columns,
versioning), `[scaffold]`. Facts:

- The old file names (`rok-data.toml`, `rok-data-sqlc.toml`) remain
  readable compat aliases; reading one prints a notice.
- Legacy alias commands (ADR 0012) keep reading their legacy file names
  unchanged.
- Flags beat the file, everywhere, as today; unknown keys are denied;
  `rok_data_version` / `rok_data_path` stay mutually exclusive.
- Each engine reads only its own sections of `rok.toml` — which amends
  ADR 0006's "generators never read each other's config" to "generators
  never read *each other's sections*".

## Consequences

- `rok init` writes `rok.toml`; the templates ship it filled in.
- The `[conventions]` key set feeds both the ORM derive generation and
  the engine's conventions/preset machinery, so the two can no longer
  silently disagree about tenant, soft-delete, or managed columns.
