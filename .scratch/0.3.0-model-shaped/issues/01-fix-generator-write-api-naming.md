# 01: Fix generator write API naming (create / patch / destroy)

**What to build:** The generated model's static write verbs use the spec's naming — `create`, `patch`, `destroy` — instead of the current `create`, `update`, `delete`, `restore`, `delete_permanently`. A caller writes `User::create(&db, input)`, `User::patch(&db, id, patch)`, `User::destroy(&db, id)` and gets the same behaviour the engine already provides, just with the model-shaped names.

**Blocked by:** None (can start immediately).

**Status:** done

- [x] Generator emits `create` (already ✅), `patch` (replaces `update`), `destroy` (replaces `delete`) on the generated model struct
- [x] `patch` signature: `async fn patch(executor, id: UserId, input: UpdateUser) -> Result<Option<User>>` — returns `None` when row missing; empty patch returns current row
- [x] `destroy` signature: `async fn destroy(executor, id: UserId) -> Result<u64>` — returns deleted count (matches `Select::delete()`)
- [x] Thin wrapper functions in `write.rs` call the model methods so API features and hand-written services keep compiling
- [x] Templates (actix/axum) regenerate with the new method names and pass `cargo clippy -D warnings` under the template lint set
- [x] `examples/api` and compile fixtures regenerate, compile, and pass `rok gen check` with no drift