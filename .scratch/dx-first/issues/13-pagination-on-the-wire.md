# 13: Pagination on the wire — shapes, links, OpenAPI, cursor lists

**What to build:** Generated list endpoints and hand-written handlers answer pagination the way an API client expects, in one line from a `Page`/`SimplePage`/`CursorPage`:

- **Shapes**, chosen per project in `rok.toml [generate]` (default stays the current compact shape, byte-compatible per 0.3.0 D6):
  - `compact` — today's `{items, page, per_page, total}`
  - `laravel` — `{data, current_page, per_page, total, last_page, from, to, first_page_url, last_page_url, next_page_url, prev_page_url, path}`; simple pages omit `total`/`last_page`; cursor pages `{data, per_page, next_cursor, prev_cursor, next_page_url, prev_page_url, path}`
  - `resource` — rok-fluent's `{data, meta: {...}, links: {first, last, prev, next}}`
- **Links**: built from the request path, preserving the query string (Laravel's `withQueryString`), with a configurable page parameter name (`pageName`) and optional fragment
- **Generated cursor lists**: a table can opt into cursor listing (`?cursor=…&per_page=…`) in `[api]`
- **OpenAPI**: generic schemas for each page shape, referenced by every list operation
- the `paginate` runtime types gain `Serialize` for the chosen shape behind the web features, so hand-written handlers match generated ones

**Blocked by:** 05, 10, 11

**Status:** ready-for-agent

- [ ] Three shapes, selected by one `rok.toml` key; default unchanged on the wire (snapshot)
- [ ] Link building keeps other query parameters; page parameter name configurable
- [ ] Cursor listing option for generated list endpoints
- [ ] OpenAPI schemas per shape; `docs/cli-reference.md` documents the new keys
- [ ] CLI suites + fixture compile gates; contract tests for each shape's JSON
