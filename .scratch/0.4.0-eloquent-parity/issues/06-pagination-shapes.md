# 06: Pagination shapes — simple_paginate, Laravel JSON, cursor prev, OpenAPI

**What to build:**
1. **`SimplePage<T>`** — returned by `Select::simple_paginate(page, per_page) -> Result<SimplePage<T>>`. One query (no `COUNT(*)`). Struct: `{ items: Vec<T>, has_more: bool, page: u64, per_page: u64 }`. `has_more = items.len() == per_page`.
2. **`Page<T>` Laravel JSON serialization** — behind `web` feature: `Serialize` impl outputs `{ total, per_page, current_page, last_page, first_page_url, last_page_url, next_page_url, prev_page_url, path, from, to, data: [...] }`. `Page::build_urls(base, page_param, extra, fragment) -> PageUrls` helper for handlers.
3. **`CursorPage<T>` with `prev`** — add `prev: Option<Cursor>`; serialization: `{ data: [...], next_cursor, prev_cursor, has_more, per_page }`.
4. **Handler responses** — generated `handlers.rs` returns `PaginatedResponse<T>` / `CursorResponse<T>` using the above serialization.
5. **OpenAPI** — generic `PaginatedResponse<T>` and `CursorResponse<T>` schemas via UTOIPA; each feature references them.

**Blocked by:** 01 (ModelCollection for fresh), 02 (simple_paginate uses same query logic).

**Status:** superseded — by `.scratch/dx-first/issues/09`–`13` (pagination vocabulary, offset, cursor both ways, batch iteration, wire shapes)

- [ ] `SimplePage<T>` in `page.rs` with `has_more()` and `map`
- [ ] `Page::build_urls` and `Serialize` (Laravel shape) behind `web` feature
- [ ] `CursorPage::prev` field + updated `Serialize`
- [ ] `Select::simple_paginate` implementation (reuses `paginate_sql` without count query)
- [ ] Generated `PaginatedResponse<T>` / `CursorResponse<T>` in handler templates
- [ ] OpenAPI generic schemas via `#[schema(generics(...))]`
- [ ] Facade tests: simple_paginate, JSON shape, cursor prev/next
- [ ] Module fixtures for paginated handlers