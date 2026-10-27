# 10: Eager loading enhancements — nested with, load/load_missing on Loaded, constrain for morphTo

**What to build:**
1. **Nested `Include`** — `Include` gains `children: Vec<Include>` so `Select::with(Author::POSTS.with(Post::COMMENTS))` works. The loader recursively fetches each level in batched queries.
2. **`Loaded::load(relation, closure)`** — lazy-load a relation on an already-loaded set: `let loaded = users.load(User::POSTS, |q| q.filter(Post::PUBLISHED.eq(true))).await?;`. Runs batched query for the relation on the loaded parents.
3. **`Loaded::load_missing(relation)`** — same but skips if relation already loaded.
4. **`Include::constrain(closure)` for `morphTo`** — when eager-loading a polymorphic relation, `with(Comment::COMMENTABLE.constrain(...))` shapes each type's load query.

**Blocked by:** 03 (polymorphic `morphTo` not in 0.4.0 — but `constrain` API can be added for future use; nested includes work with existing relations).

**Status:** done (2026-10-27) — one item dropped by decision, below

- [x] Nested includes: `Author::POSTS.with(Post::COMMENTS)` (and `Include::new(rel).filter(…).with(…)`), any depth, several per level; each level is one batched query over all of the previous level's records. Read back with `view.nested(Author::POSTS)`, a `Loaded<Post>` of every post the relation loaded, whose `get` / `try_get` read the nested relations. The target model must be `Clone` (its rows are cloned into the nested view). `RelationAggregates` became `RelationExt` (still unreleased) since it now carries `with` too
- [x] `Loaded::load(&db, includes)` (always loads, replacing an earlier load of the same relation) and the existing `load_missing` (skips what is parked); shaping goes through `Include` (`filter`, `order_by`, `limit`) rather than a closure, as `Select::with` takes it
- [x] `Include::constrain` for polymorphic relations — **dropped by decision:** polymorphic relations are out of 0.4.0, and a stub with no behavior would be API without meaning; it comes with them
- [x] Facade tests: `tests/features/nested.rs` (nested has-many, belongs-to, two levels, `load` vs `load_missing`)
- [x] Module fixture expectations — not applicable: the generator emits nothing for it
