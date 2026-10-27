# 04: Named scopes with parameters + multiple global scopes

**What to build:**
1. **Parameterized scopes**: `#[rok(scopes(of_type = path::to::fn))]` where the free function takes `(&Select<User>, UserType) -> Select<User>`. The derive emits `User::of_type(ty) -> Select<User>` and `Select::of_type(ty)` via the `Scope` trait.
2. **Multiple global scopes**: `#[rok(scoped_by = "path::to::fn")]` repeatable. Each adds a global scope; `Select::unscoped()` removes all. Scopes compose: `User::popular().active()` works with both named and global scopes.
3. **Strict mode**: `Model::prevent_lazy_loading()` makes `Loaded::try_get` return `Result<&T, LazyLoadError>` instead of `Option` when a relation wasn't in the `with` chain.

**Blocked by:** 03 (scopes can reference through/many-to-many relations).

**Status:** done (2026-10-27) — items reshaped by decision, below

- [x] Scopes — **already done by dx-first ticket 06 and 0.3.0 ticket 03, differently:** a scope takes the chain by value (`fn(Select<M>, …) -> Select<M>`); parameters are declared in the attribute (`#[rok(scopes(older_than(n: i32) = path))]`); the derive writes a `{Model}Scopes` trait for the chain method. No separate `Scope` trait: a plain function is the scope.
- [x] Macro parses `scopes(name = path, ...)` and emits model method + chain method (as above)
- [x] Multiple global scopes — **by decision, the existing `default_scope` attribute is repeatable** rather than a second name (`scoped_by`) for the same concept: `#[rok(default_scope = a, default_scope = b)]` applies both, combined with `AND`; `.unscoped()` drops them together
- [x] Strict mode — **changed by decision:** the runtime never loads a relation implicitly (`user.posts()` is a chain you await), so there is no lazy load to prevent. The real mistake is reading a relation the query didn't load, and the fix is local, not a process-wide flag: `Loaded::get(rel, row)` returns `Result`, failing with `Error::NotLoaded` (whose message names `.with(…)` / `load_missing`); `try_get` keeps its `Option`
- [x] Facade tests: parameterized scopes (existing), repeated default scopes (`scopes.rs`), `Loaded::get` on an unloaded relation (`has.rs`)
- [x] Module fixture expectations for generated scopes (existing, from the engine's `[scopes]` configuration)
