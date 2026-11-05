# 46: Serving a new table is announced, and `[api] include`

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** when `rok gen` serves a table for the first time (not in the previous snapshot), it prints a loud line: "now serving `/api/v1/<table>` (list, create, update, delete); add it to `[api] exclude` to keep it private". `[api] include` is an allowlist alternative to `exclude`; setting both is a configuration error. The default stays "serve all".

**Blocked by:** None (can start immediately)

**Status:** done

- [x] A new table prints the line once; the next run doesn't (first time = the feature directory under `api.output` holds no generated `mod.rs` before the run)
- [x] `include` serves only the named tables; `include` + `exclude` fails
- [x] Docs: the configuring guide and the template READMEs mention `include` (the engine README's reference, the CLI reference and the `init` skeleton's `rok-data-sqlc.toml`; the rok-templates READMEs live in that member repo)
