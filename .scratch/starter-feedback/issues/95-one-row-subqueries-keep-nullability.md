# 95: A one-row scalar subquery keeps its column's nullability

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** a scalar subquery `(SELECT <expr> FROM …)` is non-null when it always returns one row and `<expr>` is non-null. It always returns one row when it has an aggregate, no `GROUP BY`, no `HAVING`, no `LIMIT` or `OFFSET`, and no set operation. So `(SELECT count(*) FROM passes)` and `(SELECT coalesce(sum(x), 0)::bigint FROM …)` are `i64`. Any other scalar subquery stays `Option`. A query's untyped-column failure keeps pointing at `-- column: <name> <RustType>`, which already overrides nullability; no new annotation.

Karibu Pass's `PlatformMetrics` is one row of such subqueries, all `Option<i64>` today, and its service unwraps fourteen of them (ticket 28).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `(SELECT count(*) FROM products)` infers `i64`; with `GROUP BY` or `LIMIT` it stays `Option<i64>` (analyzer tests)
- [ ] A subquery over a nullable aggregate (`(SELECT sum(stock)::bigint FROM products)`) stays `Option<i64>`
