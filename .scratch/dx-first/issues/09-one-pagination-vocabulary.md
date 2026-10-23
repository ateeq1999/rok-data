# 09: One pagination vocabulary (prefactor)

**What to build:** A user meets one set of pagination types, the same in the runtime and in generated code. Today there are three unrelated "page" concepts with three integer widths:

| Where | Type | Meaning | Counts |
|---|---|---|---|
| runtime | `Page<T>` | a page of results | `u64` |
| generated `shared/` | `Pagination`, `PageQuery`, `PaginatedResponse<T>` | request + response | `u32` page, `i64` total |
| generated `db/search` | `Page` | a limit/offset window | `i64` |

Make the runtime's `Page<T>` (and `CursorPage<T>`) the one result type; generated `PaginatedResponse<T>` becomes the wire form built `From<Page<T>>`; the generated limit/offset struct is renamed so `Page` means one thing; one integer width for page numbers and counts. No new pagination behavior in this ticket — later tickets add it on this base.

**Blocked by:** 03

**Status:** done (one item by decision, below)

- [x] A runtime `Page<T>` becomes the wire response in one call: generated `From<rok_data::Page<T>> for PaginatedResponse<T>` (test in api-app `shared`). The generated list endpoints keep their own SQL (search filters, sorts) and build the response from it as before.
- [x] `db/search`'s window type no longer named `Page`
- [x] ~~One width~~ — **decided against**: the runtime `Page<T>` (`u64`) shipped in 0.2.0, and the wire types (`u32` page/per_page, `i64` total) are pinned by the OpenAPI contract (`int32`/`int64`); either change breaks released users or API clients. The conversion is defined once (saturating) instead.
- [x] Wire JSON byte-identical to today (snapshot proves it)
- [x] Glossary entries: Page, CursorPage, Cursor, and the window type
