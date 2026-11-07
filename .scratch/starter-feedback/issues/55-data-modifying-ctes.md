# 55: Data-modifying CTEs

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** typed queries support data-modifying CTEs (`WITH x AS (UPDATE … RETURNING …) INSERT … SELECT … FROM x`), typed from the final statement; the query's kind (`:one`, `:many`, `:execrows`) applies to it, and a CTE's `RETURNING` columns are known to the statements after it. With ticket 49, a checkout like Karibu's `CompleteCheckout` fits in one statement.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] An `UPDATE … RETURNING` CTE feeding an `INSERT … SELECT` generates with `:execrows` and with `RETURNING`
- [x] Two chained data-modifying CTEs generate
- [x] DB-backed test runs the statement atomically
