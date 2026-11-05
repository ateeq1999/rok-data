# 54: `UPDATE … FROM`, `DELETE … USING`, and `-- rok:skip`

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** typed queries support `UPDATE … FROM` and `DELETE … USING`, with parameters and `RETURNING` typed like the equivalent join. A query marked `-- rok:skip` is left ungenerated on purpose and is not an error under ticket 20's strict default, without `--lenient` for the whole run.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] Karibu's `HoldRooms` shape (`UPDATE allocations … FROM offers o JOIN partners p …`) generates, `:execrows` and `RETURNING`
- [x] `DELETE … USING` generates
- [x] `-- rok:skip` leaves a query out with a note, not an error
