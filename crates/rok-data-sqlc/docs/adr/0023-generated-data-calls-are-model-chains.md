# Generated data calls are model methods that return chains

Date: 2026-10-28 · Status: accepted

The generated `db/` folder used two styles at once. CRUD went through the runtime (`User::filter(User::ID.eq(id)).first(executor)`), while `search`, `search_after`, the list endpoint's `listing.rs` and every annotated-query function built SQL with `sqlx::QueryBuilder` or `sqlx::query_as`. So generated code returned two error types (`rok_data::Error` and `sqlx::Error`), paged with a local `Window`/`Limit` beside the runtime's `Page`/`CursorPage`, and reached its siblings through `super::super::` paths. The runtime now expresses everything those functions do, so this ADR moves all generated data code onto it, and it changes the generated shape that engine ADRs 0006 and 0007 describe.

**Calls are model methods.**

- **The shape:** a table's reads and writes are associated functions on its model, `User::find(id)`, `User::by_email(email)`, `User::search(filter)`, rather than free functions in `read.rs`/`write.rs`/`listing.rs`.
- **They return chains:** a lookup or search returns a `Select<User>` (or the runtime's write builder). The call site picks how to run it: `.first(&db)`, `.all(&db)`, `.count(&db)`, `.paginate(&db, page, per_page)`, `.cursor_paginate(..)`. One generated function covers what `search`, `search_after`, `list`, `list_page` and `count` did.
- **Annotated queries stay free functions:** an annotated query has no natural owner model, so `db::queries::<file>::<query>` keeps its place (engine ADR 0008).

**Generated code never names `sqlx`.**

- **Model calls:** they go through the query chain.
- **Annotated queries:** they go through `rok_data::raw(sql).bind(..)`, so their functions return `rok_data::Error` too.
- **Filters and sort keys:** they build `rok_data` conditions and orders instead of pushing SQL text into a `QueryBuilder`.
- **The one error type:** `rok_data::Error`. The templates map it to HTTP once (`From<rok_data::Error> for AppError`): not found `404`, unique violation `409`, foreign-key or check violation `422`, serialization failure or deadlock `503` with `Retry-After`, anything else `500` (logged, not echoed).

**List endpoints answer `Page<T>`.**

- **The default:** a generated list endpoint answers a `Page<T>`, its items and the total, from one round trip.
- **The other modes:** `SimplePage` and `CursorPage` stay on the chain, one method away.

**The folder is smaller.**

- **The files:** `db/<table>/` holds:
  - `model.rs`: the struct, the typed id and the column constants;
  - `queries.rs`: the `impl` block of calls;
  - `filter.rs`: the filter and sort-key types;
  - `relationships.rs`.
- **One import:** `crate::db::prelude` re-exports the models, their columns and the paging types, so a handler needs one `use`.
- **Names, not paths:** a generated file imports each model and typed id it uses once, and its signatures and bodies name them bare (`Result<Option<Media>, rok_data::Error>`). The imports stay relative (`super::…`). Without `[api]`, the engine doesn't know the data layer's Rust path, since only `api.db_path` names it.

## Considered Options

- **Keep free functions, tidied.** Rejected: the call site stays `users::read::get_by_id(&db, id)`, and each paging mode stays a separate generated function.
- **Both, with free functions as thin wrappers over the methods.** Rejected: it doubles the generated surface this change shrinks.
- **Keep `sqlx` in annotated-query functions.** Rejected: one error type is the point, and `rok_data::raw` covers the same SQL.
- **Deprecated shims for the old names.** Not done: the output is regenerated, not maintained by hand, and the workspace is before 1.0.

## Consequences

- Regenerating an existing project changes signatures: the error type, the paging types, and the calls' names and owners. The changelog and `docs/migration-guide.md` say how to move a call site.
- `db/search.rs` (`Window`, `Limit`, `Sort`) is no longer generated.
- The milestone snapshots and the templates' generated files refresh through `ROK_DATA_SQLC_UPDATE_EXPECTED=1`.
