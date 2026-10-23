# 14: Every documented call compiles

**What to build:** A user can paste any example from getting-started, the migration guide or the pagination guide and it compiles against the current code. Today the examples are marked `ignore` and several are wrong: `author.posts()` on an `Option<Post>`, `Note::popular()` on a model without scopes, a claim that generated models carry `create/patch/destroy`, a `load_missing` example passing `Post::COMMENTS` to `Loaded<User>`, a `chunk` closure that cannot compile, and "only the raw identifier compiles" right above `where_` code.

**Blocked by:** 03, 04, 06, 07, 08, 10, 11, 12, 13

**Status:** ready-for-agent

- [ ] Docs examples run as compiled doctests (or a docs-test crate) — no `ignore` without a written reason
- [ ] Migration guide lists the model-shaped renames: `users::write::create → User::create`, `read::get_by_id → User::find`, scope signature, `update_only`, not-found convention
- [ ] A pagination guide with the Laravel → rok-data table (offset, simple, cursor both ways, batch iteration, wire shapes)
- [ ] AGENTS.md ADR counts corrected (engine series and workspace series)
