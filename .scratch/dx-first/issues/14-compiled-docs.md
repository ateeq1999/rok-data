# 14: Every documented call compiles

**What to build:** A user can paste any example from getting-started, the migration guide or the pagination guide and it compiles against the current code. Today the examples are marked `ignore` and several are wrong: `author.posts()` on an `Option<Post>`, `Note::popular()` on a model without scopes, a claim that generated models carry `create/patch/destroy`, a `load_missing` example passing `Post::COMMENTS` to `Loaded<User>`, a `chunk` closure that cannot compile, and "only the raw identifier compiles" right above `where_` code.

**Blocked by:** 03, 04, 06, 07, 08, 10, 11, 12, 13

**Status:** done

- [x] Docs samples compile: each names its compiled source (`<!-- source: … -->`, sources in `crates/rok-data/tests/docs` and the fixture app's `one_import`), checked by `crates/rok-data/tests/docs`; the only `ignore`s are migration "before" blocks marked `<!-- not compiled: 0.2 code … -->`
- [x] Migration guide lists the model-shaped renames: `users::write::create → User::create`, `read::get_by_id → User::find`, scope signature, `update_only`, not-found convention
- [x] A pagination guide with the familiar-name → rok-data table (offset, simple, cursor both ways, batch iteration, wire shapes)
- [x] AGENTS.md ADR counts corrected (engine series and workspace series)
