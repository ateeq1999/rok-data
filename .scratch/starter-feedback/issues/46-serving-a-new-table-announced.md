# 46: Serving a new table is announced, and `[api] include`

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** when `rok gen` serves a table for the first time (not in the previous snapshot), it prints a loud line: "now serving `/api/v1/<table>` (list, create, update, delete); add it to `[api] exclude` to keep it private". `[api] include` is an allowlist alternative to `exclude`; setting both is a configuration error. The default stays "serve all".

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A new table prints the line once; the next run doesn't
- [ ] `include` serves only the named tables; `include` + `exclude` fails
- [ ] Docs: the configuring guide and the template READMEs mention `include`
