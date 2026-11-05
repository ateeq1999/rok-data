# 48: Better parameter names

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in query parameter naming, `<column> BETWEEN $2 AND $3` names the bounds `<column>_from` and `<column>_to` (instead of `<column>` and `<column>_2`), and `-- param: $2 from` renames a parameter without restating its type. A trailing `!` (`-- param: $2 reviewer!`) marks it not-null, keeping its inferred type, for a value written to a nullable column that the query always sets.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `starts_on BETWEEN $2 AND $3` generates `starts_on_from`, `starts_on_to`
- [x] `-- param: $2 from` keeps the inferred type
- [x] `-- param: $2 reviewer!` on a nullable column's value gives `Uuid`, not `Option<Uuid>`
- [x] Existing generated names elsewhere unchanged (fixtures)
