# 78: Subqueries in `FROM`

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** typed queries support a derived table in `FROM` (`FROM (SELECT …) x`, with an optional column alias list `x(a, b)`), and `LATERAL (SELECT …) x`. Its columns are typed from the inner query, as a CTE's are, and the inner query sees the outer tables only when `LATERAL`. This works in `SELECT`, in `UPDATE … FROM`, in `DELETE … USING`, and in a data-modifying CTE. Today it fails "only tables and set-returning functions are supported in FROM (no subqueries)".

Karibu Pass's `CancelEvent` needs it: its CTE `t` is `UPDATE ticket_tiers SET … FROM (SELECT tier_id, count(*) …) x WHERE …`. Writing `x` as a CTE works until then.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `CancelEvent`'s shape, with `FROM (SELECT …) x` inside a data-modifying CTE, generates (test)
- [x] `LATERAL (SELECT …)` sees the outer row; a non-`LATERAL` subquery referring to an outer table fails naming it
- [x] Column alias lists rename the derived columns
