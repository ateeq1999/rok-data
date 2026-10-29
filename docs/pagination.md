---
title: Pagination
description: Offset pages, cursor pages, chunking a table, streaming rows, and the JSON shapes pages answer on the wire.
updated: 2026-10-29
kind: reference
source: crates/rok-data/tests/docs/pagination.rs (compiled samples)
---

# Pagination

## Agent brief

- **What this page does:** the paging surface — `paginate` /
  `page` / `simple_paginate` / `for_page`, cursor pagination, chunking
  and streaming, and the `serde` wire shapes (`paginator(links)`,
  `resource(links)`).
- **Pick by need:** total + numbers → `paginate`; model's default size →
  `page` (`#[rok(per_page = N)]`, 15 default); no `COUNT(*)` →
  `simple_paginate`; stable-under-inserts → `cursor_paginate`; bulk
  process → `chunk_by_id` / `lazy_by_id`.
- **Constraints:** pages are 1-based; cursor sort keys must be `NOT
  NULL` (the primary key is the tiebreaker); `per_page = 0` is an error.
- **Samples are compiled** (`crates/rok-data/tests/docs/pagination.rs`),
  marked `<!-- source: … -->` — edit that file, not this page.

Three ways to page through a query, a way to walk a whole table, and the
JSON shapes a page answers in. Every sample below is compiled
(`crates/rok-data/tests/docs`).

| What you want | Call | Returns |
| ------------- | ---- | ------- |
| a page and the total | `.paginate(db, page, per_page)` | `Page<M>`: `items`, `total`, `page`, `per_page`, and `last_page()`, `first_item()` / `last_item()`, `on_first_page()` / `on_last_page()`, `has_pages()` |
| a page at the model's size | `.page(db, page)` | `Page<M>`, at `#[rok(per_page = N)]` (15 by default) |
| a page without the total (no `COUNT(*)`) | `.simple_paginate(db, page, per_page)` | `SimplePage<M>`: `items`, `has_more()` |
| just the window | `.for_page(page, per_page)` | the chain, with `LIMIT` / `OFFSET` |
| a page with its relations | `.with(..).paginate(db, page, per_page)` | `LoadedPage<M>`: the page's numbers beside the loaded view |
| stable pages while rows are added | `.cursor_paginate(db, cursor, per_page)` | `CursorPage<M>`: `items`, `next` / `prev` cursors |
| every row, in windows | `.chunk(db, n, f)`, `.chunk_by_id(db, n, f)` | `()`; `f` gets each window |
| every row, as a stream | `.lazy(db, n)?`, `.lazy_by_id(db, n)?`, `.lazy_by_id_desc(db, n)?` | a `Stream` of rows |
| every row, one at a time | `.each(db, f)`, `.each_by_id(db, f)` | `()`; `f` gets each row |

Pages are 1-based; a page past the end is an empty page with its numbers
intact, and a `per_page` of 0 is an error.

## Offset pages

<!-- source: crates/rok-data/tests/docs/pagination.rs -->
```rust
// a page and the total: one round trip
let page = Post::query()
    .order_by(Post::ID)
    .paginate(&db, 2, 20)
    .await?;
println!(
    "rows {:?}–{:?} of {}",
    page.first_item(),
    page.last_item(),
    page.total
);

// at the model's own size: `#[rok(per_page = 25)]`, 15 without it
let page = Post::query().page(&db, 2).await?;

// no total, no `COUNT(*)`: the rows and whether more follow
let page = Post::query().simple_paginate(&db, 2, 20).await?;
if page.has_more() {
    // link to page 3
}

// a relation's chain, and a page with its relations loaded
let comments = post.comments().paginate(&db, 1, 50).await?;
let page = Post::query()
    .with(Post::COMMENTS)
    .paginate(&db, 1, 20)
    .await?;
```

## Cursor pages

A cursor holds the ordering values of a page's edge row, so the next page
starts after it however many rows were added meanwhile, and the cost does
not grow with the page number. Every page carries a `next` cursor (after
its last row) and a `prev` cursor (before its first); either goes back into
`cursor_paginate`, and the rows always come in the query's order. A cursor
round-trips through a URL-safe string for API clients:

<!-- source: crates/rok-data/tests/docs/pagination.rs -->
```rust
// a client's token parses back into a cursor (a bad one is
// `Error::InvalidCursor`)
let cursor: Option<Cursor> = token.map(|token| token.parse()).transpose()?;
let page = Post::query()
    .order_by(Post::VIEWS.desc())
    .cursor_paginate(&db, cursor.as_ref(), 20)
    .await?;
let next = page.next_cursor().map(ToString::to_string);
let prev = page.prev_cursor().map(ToString::to_string);
```

Sort keys must be `NOT NULL`; the primary key is added as a tiebreaker.

## Walking a whole table

`chunk` reads one `LIMIT`/`OFFSET` window per query (by primary key when
the query has no order), so no connection is held while the callback runs.
When the callback deletes or changes the rows it was handed, offsets shift:
use `chunk_by_id`, which starts each window after the last key instead. A
callback returns `Ok(())` to go on, or `Ok(false)` to stop.

<!-- source: crates/rok-data/tests/docs/pagination.rs -->
```rust
let db = &db;
// windows by primary key: safe to delete what you were handed
Post::filter(Post::VIEWS.eq(0))
    .chunk_by_id(db, 500, |posts| async move {
        for post in posts {
            post.delete(db).await?;
        }
        Ok(())
    })
    .await?;

// a stream of rows, read a thousand at a time
let mut posts = Post::query().lazy_by_id(db, 1000)?;
while let Some(post) = posts.try_next().await? {
    // …
}
```

## On the wire

With the `serde` feature, a page serializes as itself (`{items, total,
page, per_page, total_pages}`), and answers in two richer shapes, its links
built from the request:

- `page.paginator(links)` — `data`, `current_page`, `per_page`, `total`,
  `last_page`, `from`, `to`, and `first_page_url`, `last_page_url`,
  `next_page_url`, `prev_page_url`, `path` (a simple page leaves out the
  total and the last page; a cursor page carries `next_cursor` /
  `prev_cursor`);
- `page.resource(links)` — `{data, links: {first, last, prev, next},
  meta: {…}}`.

<!-- source: crates/rok-data/tests/docs/pagination.rs -->
```rust
use rok_data::wire::Links;

// the request's own URI: every link keeps its other query parameters
let links = Links::from_request(uri);
let paginator = serde_json::to_value(page.paginator(links.clone()))?;
let resource = serde_json::to_value(page.resource(links))?;
```

`Links::from_request(uri).page_param("p")` renames the page parameter (for
two lists on one page), and `.fragment("results")` ends every link with
`#results`.
