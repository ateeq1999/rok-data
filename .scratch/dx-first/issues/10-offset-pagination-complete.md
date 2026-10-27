# 10: Offset pagination — the whole surface

**What to build:** Every offset-pagination call a developer reaches for, on a query, on a relation, and with eager loads:

| Familiar name | rok-data |
|---|---|
| `->paginate(15)` | `.paginate(&db, page, 15)` → `Page<M>` (exists) |
| `->simplePaginate(15)` | `.simple_paginate(&db, page, 15)` → `SimplePage<M>`: one query, fetches `per_page + 1`, `has_more` — no `COUNT(*)` |
| `$model->perPage = 25` | `#[rok(per_page = 25)]` default, used by `.paginate_default(&db, page)` / generated handlers |
| `->forPage(3, 15)` | `.for_page(3, 15)` — chain sugar for `LIMIT/OFFSET` |
| `$user->posts()->paginate()` | relation queries paginate the same way |
| `->with('author')->paginate()` | `.with(..).paginate(..)` returns a page with includes loaded (rok-fluent's `paginate_with`) |
| `firstItem`, `lastItem`, `onFirstPage`, `onLastPage`, `hasPages`, `lastPage`, `through` | `Page` helpers: `first_item`, `last_item`, `on_first_page`, `on_last_page`, `has_pages`, `last_page`, `map` (exists) |
| `items()` / `getCollection()` | `Page::into_collection()` → `ModelCollection<M>` |

A requested page past the end returns an empty page with correct metadata, not an error. `per_page == 0` stays an error.

**Blocked by:** 09

**Status:** done (generator support deferred, below)

- [x] `simple_paginate` issues exactly one query: it is `for_page(..).limit(per_page + 1).all(..)` — one `SELECT`, no `COUNT(*)` (SQL shape tested through `for_page`; results DB-backed)
- [x] `#[rok(per_page = N)]` derive attribute (`Model::PER_PAGE`, 15 by default; 0 is a derive error). **Generator support deferred**: generated APIs default to `Pagination::DEFAULT_PER_PAGE` (20), part of the wire contract; switching them to the model's size changes what existing clients get — a decision for ticket 13 (wire shapes)
- [x] `for_page`, relation pagination, pagination with includes
- [x] `Page` helpers with tests for first/last/empty/past-the-end pages
- [x] Facade tests: SQL shape + DB-backed for each entry
