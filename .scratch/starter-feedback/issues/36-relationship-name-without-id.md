# 36: A foreign key without `_id` names its relationship after both

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in the engine's relationship inference, a foreign-key column with neither an `_id` nor an `_<referenced column>` suffix names its relationship `<column>_<referenced row>`: `offers.reviewed_by → users` gives `reviewed_by_user`. This no longer collides with the search filter's field for the column (the data layer derives both for every table, `[api] exclude` or not). Two such columns to one table stay distinct. A collision that remains fails with both names and the rename that fixes it.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `offers.reviewed_by UUID REFERENCES users (id)` generates, with `reviewed_by_user` (red first: today it fails)
- [ ] `created_by` and `reviewed_by` to `users` give two relationships
- [ ] Columns ending in `_id` keep today's names
