# 62: Boolean-switched queries

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** confirm with tests, fixing what fails, that a parameter cast `$2::bool` types as `bool`, and that `CASE WHEN $2 THEN … END` works in `WHERE` and in `ORDER BY` (each direction as its own `CASE`, since a `CASE` can't switch `ASC`/`DESC`). Document the pattern in the queries guide with Karibu's "upcoming or past bookings" as the example; with ticket 56 the count query shares the row type.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] One query serves both "upcoming" and "past" (DB-backed test)
- [x] Guide section with the pattern
