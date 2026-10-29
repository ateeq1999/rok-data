# ADR 0022: the templates live in their own crate

Date: 2026-10-29 · Status: accepted (amended by ADR 0027)

The actix and axum templates move from `crates/rok-data-sqlc/templates/`
to `crates/rok-templates`, so template work (capabilities,
deployment, their guides and drift check) has a crate of its own instead
of sharing the generator's. The new crate is data only: no dependencies,
a `build.rs` that embeds the template directories, and `templates()`
listing them. The engine depends on it and keeps the copy logic (copy
when absent, skip `@generated` files), because `rok sqlc init --template`
is engine grammar and its tokens reach the engine verbatim (invariant 3).
The engine's plain-init scaffold, `src/init/templates/`, becomes
`src/init/scaffold/`, so "template" means only a Template.

## Considered Options

- **The CLI owns the templates**: would put `init --template` grammar in
  rok-cli, against invariant 3.
- **Templates crate owns the copy logic**: would split `init` between two
  crates, and the templates crate would need the engine's marker rules.

## Consequences

- The name is `rok-templates`, not `rok-data-templates`: deliberately
  outside the `rok-data-*` family, like the `rok` binary itself.
- One more crate name to publish, before the engine in `release.yml`'s
  order; it counts toward crates.io's new-crate limit (see workspace ADR 0008).
- Tests that drive `rok` stay in `rok-cli/tests` (workspace ADR 0003); the
  file-only `template_drift` moves to the templates crate.
- The templates' decisions are workspace ADRs from now on (0023–0025);
  there is no third ADR series.
