# 11: Cursor pagination in both directions

**What to build:** Laravel's `cursorPaginate` parity: a page knows its next **and previous** cursor, and a client can walk either way.

```rust
let page = Post::order_by(Post::CREATED_AT.desc()).cursor_paginate(&db, cursor, 20).await?;
page.next_cursor(); page.prev_cursor(); page.has_more();
```

- `cursor` argument accepts `Option<&Cursor>` or a client token directly (parse error → `Error::InvalidCursor`, mapped to 400 by generated handlers), so handlers don't hand-parse
- a cursor records its direction; following a `prev` cursor reverses the keyset comparison and the order, then restores the page's order
- the argument is named `per_page` like offset pagination (today `limit`)
- works for wrapped keysets (expression orders, joins) as it does today for `next`

**Blocked by:** 09

**Status:** ready-for-agent

- [ ] `CursorPage` carries `next` and `prev`; first page has no `prev`, last page no `next`
- [ ] Round-trip test: forward three pages, back two, items identical to the forward pages
- [ ] Wrapped-keyset (expression order, joined) prev/next tests
- [ ] Token parse failures are a typed error, not a panic or generic error
