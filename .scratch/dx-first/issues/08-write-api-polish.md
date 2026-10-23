# 08: Write API polish

**What to build:** Write verbs take what a user naturally has in hand:

- `user.update_only(&db, [User::NAME])` instead of `update_only(&db, Some(vec![User::NAME]))` — same `impl IntoIterator<Item = Column<Self>>` shape `upsert_on` already uses
- the hidden `delete_by_key(&self, exec, force: bool)` stops being callable on every model
- `insert` / `upsert` / `upsert_on` share one internal path (no three copies), with a named conflict policy instead of a bare `bool`

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] `update_only` takes `impl IntoIterator<Item = Column<Self>>`; an empty list is an error, not a silent full update
- [ ] `delete_by_key` is crate-private
- [ ] One internal insert path; behavior unchanged (existing facade tests pass untouched)
- [ ] CHANGELOG entry for the `update_only` signature
