# 42: Shared read queries

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** `queries/shared/*.sql` generates `db::queries::shared`, which any feature may call. Only read kinds (`:one`, `:many`, `:maybe_one`) are allowed there; a write kind is a generation error naming the query. `check-arch` allows calls to `shared` from any feature.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] A shared query is generated and callable from two features; check-arch passes
- [x] A `:exec` in `shared/` fails, naming it

**Done:** per-file submodules, `queries/shared/membership.sql` →
`db::queries::shared::membership`, for symmetry with the per-feature
modules: query names stay scoped by file, so two shared files may each
define `Count`, and no merging rule is needed. A read is any `:one`,
`:many` or `:maybe_one` whose statement is not an INSERT, UPDATE, DELETE
or MERGE (a data-modifying CTE already fails the analysis); a write fails
the run even with `--lenient`. `shared` is reserved, so `queries/shared.sql`
fails too. check-arch's `cross-feature-query` skips `queries::shared`
explicitly; rule 4's text is unchanged, since the published templates'
`AGENTS.md` (rok-templates 0.6.0) must state it word for word, and the
architecture guide states the exemption beside it. The axum fixture's
notes and labels services both call `queries/shared/accounts.sql`, and the
fixture passes clippy, its tests and check-arch.
