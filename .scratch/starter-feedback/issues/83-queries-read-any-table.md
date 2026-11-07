# 83: Say plainly that a feature's queries may read any table

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** check-arch rule 4 already lets a feature's own queries join and read any table, and check-arch doesn't read SQL. Its wording is mostly about writes, though, and read as forbidding it. Karibu Pass made a second call for `events.cancel_reason` instead of joining it (ticket 24).
- **Rule 4's message and the architecture guide:** "a feature's queries may join and read any table; writing another feature's table goes through that feature's service".
- **An example:** the guide shows a Pass query selecting the event's `cancel_reason`.
- **The templates' `AGENTS.md`:** its rule text is matched against the published rok-templates crate, so it changes with a rok-templates release.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] The new message and guide text (test for the message)
- [ ] The guide's example query generates
