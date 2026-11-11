# 83: Say plainly that a feature's queries may read any table

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** check-arch rule 4 already lets a feature's own queries join and read any table, and check-arch doesn't read SQL. Its wording is mostly about writes, though, and read as forbidding it. Karibu Pass made a second call for `events.cancel_reason` instead of joining it (ticket 24).
- **Rule 4's message and the architecture guide:** "a feature's queries may join and read any table; writing another feature's table goes through that feature's service".
- **An example:** the guide shows a Pass query selecting the event's `cancel_reason`.
- **The templates' `AGENTS.md`:** its rule text is matched against the published rok-templates crate, so it changes with a rok-templates release.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] The new message and guide text (test for the message): every rule-4 message (`rule-4`, `cross-feature-write`, `cross-feature-query`) carries the sentence, tested in `check_arch/tests.rs`; the guide says it under "Reads cross features; writes don't". The templates' `AGENTS.md` rule text is unchanged: it waits on a rok-templates release (the numbered rules are held verbatim against the released crate).
- [x] The guide's example query generates: `TasksWithProject` in `docs/examples/api/queries.sql` (a task joined to its project's name; the guide notes a pass reads its event's `cancel_reason` the same way), generated and compiled with the example.
