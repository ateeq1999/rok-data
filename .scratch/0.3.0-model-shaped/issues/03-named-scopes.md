# 03: Named scopes (Scope trait + #[rok(scopes(...))] attribute)

**What to build:** A model declares reusable query fragments by path: `#[rok(scopes(popular = path::to::popular, active = path::to::active))]`. The derive emits a chainable entry on the model (`User::popular() -> Select<User>`) and a same-named method on `Select` via a `Scope` trait, so scopes compose: `User::popular().active().all(&db).await?`. Parameterized scopes work: `scopeOfType(&Select<User>, UserType) -> Select<User>` emits `User::of_type(ty)`.

**Blocked by:** 01 (write API naming must be settled so scope entry points don't collide with static verbs), 02 (relations on model — scopes can reference relations).

**Status:** ready-for-agent

- [ ] Runtime exports a `Scope` trait in the prelude: `trait Scope<M: Model> { fn apply(self, q: Select<M>) -> Select<M>; }` with a blanket impl for `fn(&Select<M>) -> Select<M>`
- [ ] `Select` gets a method for each scope via the trait, so `User::popular()` and `q.popular()` both work
- [ ] Macro parses `#[rok(scopes(name = path::to::fn, ...))]` and emits:
  - On the model: `pub fn popular() -> Select<Self> { popular(Self::query()) }`
  - On `Select`: `impl Scope<User> for fn(&Select<User>) -> Select<User> { ... }` so `q.popular()` compiles
- [ ] Parameterized scopes: free function takes extra args; derive emits model method with same signature forwarding to the free function
- [ ] Global scope (`default_scope`) and `.unscoped()` unchanged — named scopes compose on top of the global scope
- [ ] Generated models can declare scopes (engine emits them when the schema has a convention or annotation); hand-written models write the attribute themselves
- [ ] Tests: facade integration suite proves composition (`User::popular().active().all(&db)`), parameterized scopes, and that `unscoped()` removes global but not named scopes