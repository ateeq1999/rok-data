# 09: One pagination vocabulary (prefactor)

**What to build:** A user meets one set of pagination types, the same in the runtime and in generated code. Today there are three unrelated "page" concepts with three integer widths:

| Where | Type | Meaning | Counts |
|---|---|---|---|
| runtime | `Page<T>` | a page of results | `u64` |
| generated `shared/` | `Pagination`, `PageQuery`, `PaginatedResponse<T>` | request + response | `u32` page, `i64` total |
| generated `db/search` | `Page` | a limit/offset window | `i64` |

Make the runtime's `Page<T>` (and `CursorPage<T>`) the one result type; generated `PaginatedResponse<T>` becomes the wire form built `From<Page<T>>`; the generated limit/offset struct is renamed so `Page` means one thing; one integer width for page numbers and counts. No new pagination behavior in this ticket — later tickets add it on this base.

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] Generated handlers build their list response from the runtime `Page<T>` in one call
- [ ] `db/search`'s window type no longer named `Page`
- [ ] Page numbers and counts use one width across runtime and generated code
- [ ] Wire JSON byte-identical to today (snapshot proves it)
- [ ] Glossary entries: Page, CursorPage, Cursor, and the window type
