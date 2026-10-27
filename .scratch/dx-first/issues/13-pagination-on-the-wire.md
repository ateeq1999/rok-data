# 13: Pagination on the wire — shapes, links, OpenAPI, cursor lists

**What to build:** Generated list endpoints and hand-written handlers answer pagination the way an API client expects, in one line from a `Page`/`SimplePage`/`CursorPage`:

- **Shapes**, chosen per project in `rok.toml [generate]` (default stays the current compact shape, byte-compatible per 0.3.0 D6):
  - `compact` — today's `{items, page, per_page, total}`
  - `paginator` — `{data, current_page, per_page, total, last_page, from, to, first_page_url, last_page_url, next_page_url, prev_page_url, path}`; simple pages omit `total`/`last_page`; cursor pages `{data, per_page, next_cursor, prev_cursor, next_page_url, prev_page_url, path}`
  - `resource` — rok-fluent's `{data, meta: {...}, links: {first, last, prev, next}}`
- **Links**: built from the request path, preserving the query string, with a configurable page parameter name (`pageName`) and optional fragment
- **Generated cursor lists**: a table can opt into cursor listing (`?cursor=…&per_page=…`) in `[api]`
- **OpenAPI**: generic schemas for each page shape, referenced by every list operation
- the `paginate` runtime types gain `Serialize` for the chosen shape behind the web features, so hand-written handlers match generated ones

**Blocked by:** 05, 10, 11

**Status:** done — the runtime half; split, by decision, into 15 (generated shape key + OpenAPI) and 16 (generated cursor lists): each touches handler generation for both frameworks and deserves its own change

- [x] Three shapes for hand-written handlers: `page.paginator(links)` and `page.resource(links)` on `Page`, `SimplePage` and `CursorPage` (`rok_data::wire`); each type's own `Serialize` stays the compact shape. The `rok.toml` key for generated endpoints → ticket 15
- [x] Link building keeps other query parameters; page parameter name (`page_param`) and `fragment` configurable (`Links::from_request`)
- [ ] Cursor listing option for generated list endpoints → ticket 16
- [ ] OpenAPI schemas per shape → ticket 15
- [x] Tests pin each shape's JSON (facade `web` tests) and the link building
