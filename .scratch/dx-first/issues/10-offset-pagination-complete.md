# 10: Offset pagination — the whole Laravel surface

**What to build:** Every offset-pagination call a Laravel developer reaches for, on a query, on a relation, and with eager loads:

| Laravel | rok-data |
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

**Status:** ready-for-agent

- [ ] `simple_paginate` issues exactly one query (SQL-shape test)
- [ ] `#[rok(per_page = N)]` derive attribute and generator support
- [ ] `for_page`, relation pagination, pagination with includes
- [ ] `Page` helpers with tests for first/last/empty/past-the-end pages
- [ ] Facade tests: SQL shape + DB-backed for each entry
