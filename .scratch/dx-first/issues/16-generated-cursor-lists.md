# 16: Generated cursor-paginated list endpoints

**What to build:** A table can opt its generated list endpoint into keyset pagination: `GET /books?cursor=…&per_page=…` answers a page and the cursors to the pages on either side, stable while rows are added. Split out of ticket 13. The runtime pieces exist: `Select::cursor_paginate` in both directions (ticket 11), `CursorPage` in every wire shape (ticket 13), and `Error::InvalidCursor` already maps to `400 invalid_cursor`.

- `[api] cursor = ["books", …]` (or a per-table list option) switches that table's list endpoint from `page` to `cursor`
- the generated list reads `?cursor=` (absent for the first page), parses it (`400 invalid_cursor` on garbage), and keeps the table's sort keys as the keyset order (primary key as tiebreaker)
- the response is the `CursorPage` in the project's pagination shape (ticket 15); OpenAPI documents the `cursor` parameter and the cursor page schema

**Blocked by:** 15

**Status:** done

- [x] `[api] cursor` key; `docs/cli-reference.md` updated
- [x] Generated list with `?cursor=`, both frameworks; a bad cursor is `400 invalid_cursor`
- [x] Template/fixture tests: forward and back through a cursor-listed table (DB-backed)
- [x] OpenAPI: the `cursor` query parameter and the cursor page schema

**How it shipped:** `[api] cursor = ["table", …]`. The generated
`list_by_cursor` in `db/<table>/listing.rs` is a model chain
(`Note::filter(…).filter_opt(q, matches_q).order_by(sort.key.order(…))
.cursor_paginate(…)`), scoped like the numbered list, and the sort key
gains `order(direction)`. The shared tools gain `CursorListQuery` /
`CursorQuery` and `cursor_error` (pagination.rs), plus `CursorResponse`
and the `paginator`/`resource` cursor shapes (response.rs). They are
emitted only when a table is listed, so other projects' output is
byte-identical. `POST /search` keeps paging by number.

Deviations, decided:
- A bad cursor is `400 bad_request`, not `invalid_cursor`. The
  application's `error.rs` is scaffolded once and owned by the user, so
  generated code can't rely on a new `AppError` variant. That covers a
  garbage token, the wrong number of values, and a value of another type
  (`42883`, e.g. a cursor from another `sort`).
- The generator refuses a table with no primary key, or whose sort keys
  include a nullable column (the keyset needs `NOT NULL` keys).
- Runtime fix found on the way: `impl_value!` key newtypes were
  `Value::Custom`, which a cursor refused, so `cursor_paginate` couldn't
  pass page 1 of any generated model. A custom value over a built-in
  type now travels as its plain value (`CustomValue::plain`).

Tests: `tests/api_cursor.rs` (engine), the api-app's `tests/cursor.rs`
(DB-backed: forward and back, stability while rows are added, the
default sort, `q`, and both kinds of bad cursor), the axum-app's
cursor-paginator JSON and document tests, and the runtime's newtype
round trip.
