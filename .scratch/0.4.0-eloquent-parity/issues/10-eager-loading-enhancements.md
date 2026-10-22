# 10: Eager loading enhancements — nested with, load/load_missing on Loaded, constrain for morphTo

**What to build:**
1. **Nested `Include`** — `Include` gains `children: Vec<Include>` so `Select::with(Author::POSTS.with(Post::COMMENTS))` works. The loader recursively fetches each level in batched queries.
2. **`Loaded::load(relation, closure)`** — lazy-load a relation on an already-loaded set: `let loaded = users.load(User::POSTS, |q| q.filter(Post::PUBLISHED.eq(true))).await?;`. Runs batched query for the relation on the loaded parents.
3. **`Loaded::load_missing(relation)`** — same but skips if relation already loaded.
4. **`Include::constrain(closure)` for `morphTo`** — when eager-loading a polymorphic relation, `with(Comment::COMMENTABLE.constrain(|morph| morph.constrain([Post::class => |q| q.where(...), Video::class => |q| q.where(...)])))` shapes each type's load query.

**Blocked by:** 03 (polymorphic `morphTo` not in 0.4.0 — but `constrain` API can be added for future use; nested includes work with existing relations).

**Status:** ready-for-agent

- [ ] `Include::children: Vec<Include>` + recursive loading in `WithSelect::all`
- [ ] `Loaded::load` and `load_missing` methods taking relation constant + optional closure
- [ ] `Include::constrain` storing a type-erased closure for morphTo (stub for 0.4.0, implemented when polymorphic lands)
- [ ] Facade tests: nested with, load on Loaded, load_missing
- [ ] Module fixture expectations for nested includes