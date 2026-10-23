# 15: Generated list endpoints answer in the chosen page shape

**What to build:** A project picks the JSON shape its generated list endpoints answer in, with one `rok.toml` key, and the OpenAPI document describes that shape. Split out of ticket 13, whose runtime half shipped: `Page`/`SimplePage`/`CursorPage` already answer `laravel(links)` and `resource(links)` (`rok_data::wire`), and `Links::from_request` builds the links from the request URI.

- `[api] pagination = "compact" | "laravel" | "resource"` (default `compact`: today's `{items, page, per_page, total}`, byte-identical — 0.3.0 spec D6)
- the generated handlers read the request URI (actix `HttpRequest::uri`, axum `OriginalUri`) for the links, so every link keeps the other query parameters
- the generated list endpoint builds a runtime `Page` and answers through the chosen shape (the generated `PaginatedResponse` stays the `compact` form)
- OpenAPI: one generic schema per shape, referenced by every list operation
- the default page size: today `Pagination::DEFAULT_PER_PAGE` (20) — decide whether a model's `#[rok(per_page = N)]` (ticket 10) feeds it; changing it changes what existing clients get, so only under a non-default shape or an explicit key

**Blocked by:** 13

**Status:** ready-for-agent

- [ ] `[api] pagination` key in the engine's config and `docs/cli-reference.md`, rejected with the valid values when unknown
- [ ] `compact` output and OpenAPI byte-identical to today (template contract tests and the milestone record unchanged)
- [ ] `laravel` and `resource`: generated handlers for actix and axum; contract tests pin each shape's JSON
- [ ] OpenAPI schemas per shape; fixture compile gates and CLI suites green
