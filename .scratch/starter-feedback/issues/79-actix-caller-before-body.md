# 79: actix's caller extractors run before the body

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in the actix templates, `AuthUser` and `AdminUser` hold the request body back until they've checked the caller, as ticket 44's `Allowed<G>` does. actix polls a handler's extractors together, so today a `Json` body can answer 422 before `AuthUser` answers 401, showing the body's rules to callers who can't use the route. A wrong `Content-Type` or an over-limit `Content-Length` is still rejected first, since actix checks those up front; the docs say so.

**Blocked by:** None (ticket 44 has shipped `Allowed`'s body hold)

**Status:** ready-for-agent

- [ ] An unauthenticated call with a bad body gets 401, not 422 (actix tests); a non-admin gets 403
- [ ] axum is unchanged (its extractors already run in order)
