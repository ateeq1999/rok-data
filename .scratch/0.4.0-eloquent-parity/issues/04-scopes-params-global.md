# 04: Named scopes with parameters + multiple global scopes

**What to build:**
1. **Parameterized scopes**: `#[rok(scopes(of_type = path::to::fn))]` where the free function takes `(&Select<User>, UserType) -> Select<User>`. The derive emits `User::of_type(ty) -> Select<User>` and `Select::of_type(ty)` via the `Scope` trait.
2. **Multiple global scopes**: `#[rok(scoped_by = "path::to::fn")]` repeatable. Each adds a global scope; `Select::unscoped()` removes all. Scopes compose: `User::popular().active()` works with both named and global scopes.
3. **Strict mode**: `Model::prevent_lazy_loading()` makes `Loaded::try_get` return `Result<&T, LazyLoadError>` instead of `Option` when a relation wasn't in the `with` chain.

**Blocked by:** 03 (scopes can reference through/many-to-many relations).

**Status:** ready-for-agent

- [ ] `Scope` trait in prelude: `trait Scope<M> { fn apply(self, q: Select<M>) -> Select<M>; }` with blanket impl for `Fn(&Select<M>) -> Select<M>` and for parameterized fn
- [ ] Macro parses `scopes(name = path, ...)` and emits model method + `Scope` impl
- [ ] Macro parses repeatable `scoped_by` and emits multiple `default_scope` functions composed
- [ ] `Select::prevent_lazy()` sets a flag; `Loaded::try_get` checks it
- [ ] Facade tests: parameterized scopes, multiple global scopes composition, `prevent_lazy` error
- [ ] Module fixture expectations for generated scopes