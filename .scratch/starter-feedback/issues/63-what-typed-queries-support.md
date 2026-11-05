# 63: A page on what typed queries support

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** one engine docs page, "What typed queries support", linked from the engine README's "Writing query files" and from `docs/llms.txt`. A table lists each SQL form:
- computed output columns (casts, `coalesce`, comparisons, arithmetic, `CASE`, `||`)
- functions in `SELECT` and in `WHERE`
- CTEs, data-modifying ones included
- `UPDATE … FROM` / `DELETE … USING`
- `CASE` in `ORDER BY` and boolean switches
- set-returning functions
- parameters and their overrides

For each: whether it's typed, the override when it isn't (`-- column:`, `-- param:`, `-- rok:skip`), and the test that proves it. Tickets 49, 55, 62 and 64 update their rows as they land.

**Blocked by:** None (can start immediately; describes what main supports at the time)

**Status:** ready-for-agent

- [ ] Every row links an existing test
- [ ] Linked from the README and llms.txt
