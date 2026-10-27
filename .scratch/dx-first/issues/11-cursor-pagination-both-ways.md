# 11: Cursor pagination in both directions

**What to build:** Cursor pagination both ways: a page knows its next **and previous** cursor, and a client can walk either way.

```rust
let page = Post::order_by(Post::CREATED_AT.desc()).cursor_paginate(&db, cursor, 20).await?;
page.next_cursor(); page.prev_cursor(); page.has_more();
```

- `cursor` argument accepts `Option<&Cursor>` or a client token directly (parse error → `Error::InvalidCursor`, mapped to 400 by generated handlers), so handlers don't hand-parse
- a cursor records its direction; following a `prev` cursor reverses the keyset comparison and the order, then restores the page's order
- the argument is named `per_page` like offset pagination (today `limit`)
- works for wrapped keysets (expression orders, joins) as it does today for `next`

**Blocked by:** 09

**Status:** done (one item changed by decision, below)

- [x] `CursorPage` carries `next` and `prev`; first page has no `prev`, last page no `next`
- [x] Round-trip test: forward through every page, then back through `prev` to the first, each page identical to its forward read (three sort orders incl. mixed directions; run against PostgreSQL 17)
- [x] Wrapped-keyset (expression order, joined) prev/next tests
- [x] Token parse failures are `Error::InvalidCursor` (`Cursor: FromStr`). **Changed by decision**: `cursor_paginate` keeps taking `Option<&Cursor>` rather than also a raw token — a generic argument would break inference for the documented `cursor_paginate(&db, None, 20)`; a handler parses the token with `.parse()`
