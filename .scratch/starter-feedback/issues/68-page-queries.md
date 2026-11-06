# 68: `:page` queries

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** a `:page` query kind. The query is written once, with its `WHERE` and `ORDER BY` and no `LIMIT`/`OFFSET`. The generated function:
- **Parameters:** takes the query's parameters and the page (`page`, `per_page`, as the generated API's paging does).
- **Rows:** runs the query with `LIMIT`/`OFFSET`.
- **Total:** runs `SELECT count(*) FROM (<the query without its ORDER BY>) q`.
- **Return:** `Page<Row>` (`items`, `total`, `page`, `per_page`), the same shape the generated API serves.

A `:page` query with its own `LIMIT`/`OFFSET` is an error. `count(*) OVER ()` is not used: it gives no total once the offset passes the last row.

Also, before the 0.9.0 release: a regression test that `count(*) FILTER (WHERE …)::bigint` is `i64` (ticket 31's rule).

**Blocked by:** None (can start immediately)

**Status:** done

- [x] Karibu's `PartnerBookings` shape (a range plus a four-way search) as one `:page` query returns the right items and total (DB-backed test)
- [x] An empty page past the end still reports the total
- [x] `LIMIT` in a `:page` query fails naming it
- [x] The `FILTER` regression test passes
