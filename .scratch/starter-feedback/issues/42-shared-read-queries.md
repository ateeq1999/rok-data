# 42: Shared read queries

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** `queries/shared/*.sql` generates `db::queries::shared`, which any feature may call. Only read kinds (`:one`, `:many`, `:maybe_one`) are allowed there; a write kind is a generation error naming the query. `check-arch` allows calls to `shared` from any feature.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A shared query is generated and callable from two features; check-arch passes
- [ ] A `:exec` in `shared/` fails, naming it
