# 82: One paging shape, and a pager in every Client

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:**
- **One shape per list:** every list route answers one of the engine's two shapes: page-number paging (`items`, `total`, `page`, `per_page`, the default) or cursor paging (`items`, `next`, `prev`, for tables listed in `[api] cursor`).
  - **Hand-written lists:** they return `PaginatedResponse::from(page)` from a `:page` query (ticket 68), so they page like the generated ones.
  - **Bare arrays:** only for lists bounded by nature.
- **A pager per list operation in each Client:** the contract reader recognises both shapes.
  - **Dart:** `Pager<T>` with `load()`, `more()` and a `Stream<List<T>>`.
  - **TypeScript:** an `infiniteQueryOptions` factory for `useInfiniteQuery`.
  - **Rust:** a `pages()` async iterator.
- **Template guide:** a hand-written list with `:page`.

Karibu Pass has three shapes across five list routes (`{items, has_more}`, `{items, limit, offset, total}`, and a bare array), and each cubit pages its own way (ticket 24).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Each Client emits a pager for a page-number list and a cursor list (emission tests)
- [ ] The Dart pager's `more()` stops at the last page; the TS factory's `getNextPageParam` reads both shapes
- [ ] The guide's hand-written `:page` list compiles in a template gate
