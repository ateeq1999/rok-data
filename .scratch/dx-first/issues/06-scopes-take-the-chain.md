# 06: Named scopes take the chain by value

**What to build:** A scope reads like the chain it extends:

```rust
fn popular(q: Select<User>) -> Select<User> { q.filter(User::VOTES.gt(100)) }
```

Today a scope is `fn(&Select<M>) -> Select<M>`, so every body starts with `q.clone()`, and the mismatch with `Select::scope(self, FnOnce(Self) -> Self)` forced a `Scope` trait whose method the inherent one shadows (generated code calls `Scope::scope(chain, f)`).

**Blocked by:** 02

**Status:** done

- [x] Scope functions take and return `Select<M>` by value; the derive and the engine emit/accept that shape
- [x] The shadowing `Scope` trait workaround is gone; `.scope(f)` is the one chain method
- [x] `declare_scopes!` (no users) is removed
- [x] The derive's reserved-name list is derived from (or checked against) the runtime's verb list, so a new verb cannot silently collide with a user scope
- [x] Facade tests: static entry (`User::popular()`), chained (`….active()`); from another module: fixture-app `one_import` (ticket 02)
- [x] Migration guide shows the by-value signature (scopes are new in the unreleased 0.3.0, so there is no shipped by-reference form to migrate from; the CHANGELOG states the shape)
