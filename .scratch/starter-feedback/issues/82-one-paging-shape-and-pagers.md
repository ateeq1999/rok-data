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

- [x] Each Client emits a pager for a page-number list and a cursor list (emission tests: `a_paged_list_gets_a_pager_factory` (TS), `a_paged_list_gets_a_pager` (Dart), `a_paged_list_gets_a_pages_iterator` (Rust), over `tests/api-app`'s `notes` and `labels`; the reader's `a_list_pages_by_number_or_by_cursor`)
- [x] The Dart pager's `more()` stops at the last page; the TS factory's `getNextPageParam` reads both shapes (`nextPage`, overloaded for both; checked by hand with `dart test` and `tsc` against zod 4 and TanStack Query 5, which CI has no toolchain for)
- [ ] The guide's hand-written `:page` list compiles in a template gate. **Blocked:** the template guide and its gate live in rok-templates, whose projects generate with the released `rok`; they need a rok release carrying the pagers (and `:page`, already in 0.9.0). The engine side documents the rule in `crates/rok-data-sqlc/docs/guide/wire-standard.md`.
