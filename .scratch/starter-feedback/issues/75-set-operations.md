# 75: `UNION`, `INTERSECT` and `EXCEPT`

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** typed queries support `UNION`, `UNION ALL`, `INTERSECT` and `EXCEPT` at the top level and in subqueries. Today only a recursive CTE's `UNION` is read; anything else fails "only a plain SELECT is supported (no UNION, VALUES, or nesting)".
- **Types:** each output column takes the branches' shared type, as `CASE` does, and is nullable when any branch is.
- **Mismatches:** a column count or type mismatch between branches fails, naming the column.
- **`ORDER BY`/`LIMIT`:** these apply to the whole result, as in PostgreSQL.
- **Docs:** ticket 63's page gets its row.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] Karibu's "due Partners" shape (`SELECT partner_id FROM … UNION SELECT partner_id FROM …`) generates one function
- [x] Branch mismatches fail naming the column
- [x] A `UNION` inside a subquery and an `EXCEPT` both type
