# 48: Better parameter names

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in query parameter naming, `<column> BETWEEN $2 AND $3` names the bounds `<column>_from` and `<column>_to` (instead of `<column>` and `<column>_2`), and `-- param: $2 from` renames a parameter without restating its type.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `starts_on BETWEEN $2 AND $3` generates `starts_on_from`, `starts_on_to`
- [ ] `-- param: $2 from` keeps the inferred type
- [ ] Existing generated names elsewhere unchanged (fixtures)
