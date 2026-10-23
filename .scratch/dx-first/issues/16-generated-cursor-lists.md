# 16: Generated cursor-paginated list endpoints

**What to build:** A table can opt its generated list endpoint into keyset pagination: `GET /books?cursor=…&per_page=…` answers a page and the cursors to the pages on either side, stable while rows are added. Split out of ticket 13. The runtime pieces exist: `Select::cursor_paginate` in both directions (ticket 11), `CursorPage` in every wire shape (ticket 13), and `Error::InvalidCursor` already maps to `400 invalid_cursor`.

- `[api] cursor = ["books", …]` (or a per-table list option) switches that table's list endpoint from `page` to `cursor`
- the generated list reads `?cursor=` (absent for the first page), parses it (`400 invalid_cursor` on garbage), and keeps the table's sort keys as the keyset order (primary key as tiebreaker)
- the response is the `CursorPage` in the project's pagination shape (ticket 15); OpenAPI documents the `cursor` parameter and the cursor page schema

**Blocked by:** 15

**Status:** ready-for-agent

- [ ] `[api] cursor` key; `docs/cli-reference.md` updated
- [ ] Generated list with `?cursor=`, both frameworks; a bad cursor is `400 invalid_cursor`
- [ ] Template/fixture tests: forward and back through a cursor-listed table (DB-backed)
- [ ] OpenAPI: the `cursor` query parameter and the cursor page schema
