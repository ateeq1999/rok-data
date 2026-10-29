# Generated data calls are model methods that return chains

Date: 2026-10-28 · Status: accepted

The generated `db/` folder used two styles at once. CRUD went through the runtime (`User::filter(User::ID.eq(id)).first(executor)`), while `search`, `search_after`, the list endpoint's `listing.rs` and every annotated-query function built SQL with `sqlx::QueryBuilder` or `sqlx::query_as`. So generated code returned two error types (`rok_data::Error` and `sqlx::Error`), paged with a local `Window`/`Limit` beside the runtime's `Page`/`CursorPage`, and reached its siblings through `super::super::` paths. The runtime now expresses everything those functions do, so this ADR moves all generated data code onto it, and it changes the generated shape that engine ADRs 0006 and 0007 describe.

**Calls are model methods.**

- **The shape:** a table's reads and writes are associated functions on its model, `User::by_email(email)`, `User::search(filter)`, `User::matching(q, sort)`, rather than free functions in `read.rs`/`write.rs`/`listing.rs`.
- **What the runtime already names isn't generated:** `Model` already gives every model `find`, `find_or_fail`, `all`, `count`, `exists` and `query`. So the engine stops emitting `get_by_id`, `exists_by_id`, `list`, `list_page` and `count`, and a call site uses the runtime's verb (`User::find(&db, id)`), one name per operation (workspace ADR 0020). `find` takes what converts into the model's `Model::Key`, so another table's typed id does not compile as the key; a composite key struct converts into the key tuple, so `find` takes it too. The engine generates only what the schema knows and the runtime doesn't:
  - `by_primary_key`: the key's chain;
  - `by_<columns>`: one per unique key, `User::by_email(email)`;
  - the filter's `search`;
  - the list endpoint's `matching`.
- **Why `by_primary_key` sits beside `find`:** `find(&db, id)` fetches, and `by_primary_key(id)` is the query that fetch runs, as `by_email` is for a unique column. The generated services, prechecks and `patch` need one key lookup that also checks existence (`.exists(..)`), takes a tenant first on a tenant table, and borrows a text key. `by_key` was the first name, but it collided with a unique `key` column (`roles.key`, `settings.key`), a common one. A unique lookup that would still collide (a column named `primary_key`) is skipped.
- **No module wrappers:** the 0.2 free functions in `write.rs` (`books::write::create`), deprecated in 0.3, are gone. The verbs exist only on the model.
- **Tenant tables keep explicit tenant arguments.** The runtime's tenancy (`with_tenant`) is task-local and fails closed, while generated tenant tables take the tenant as a parameter. A tenant table's generated calls take it first.
- **They return chains:** a lookup or search returns a `Select<User>` (or the runtime's write builder). The call site picks how to run it: `.first(&db)`, `.all(&db)`, `.count(&db)`, `.paginate(&db, page, per_page)`, `.cursor_paginate(..)`. One generated function covers what `search`, `search_after`, `list`, `list_page` and `count` did.
- **Annotated queries stay free functions:** an annotated query has no natural owner model, so `db::queries::<file>::<query>` keeps its place (engine ADR 0008).

**Generated queries run through `rok_data`, not `sqlx`.**

- **What stays `sqlx`:** the type mapping the runtime binds and decodes through (`sqlx::Type` and `FromRow` derives, a generated enum's `Encode`/`Decode`) and the executors (`PgPool`, `PgConnection`). These are the runtime's types, not a second query path.
- **Not yet converted:** two generated paths still run their statements through `sqlx`: the versioned `patch` (dynamic SQL, decoding the row and an `applied` flag) and the relationship loaders (joined rows decoded by prefix). They are left for a later change.
- **Model calls:** they go through the query chain.
- **Annotated queries:** they go through `rok_data::raw(sql).bind(..)`, so their functions return `rok_data::Error` too.
- **Sort keys:** a sort enum answers a typed `rok_data::Order` (`UserSort::order(direction)`). The list's `q` is `filter_any(SEARCH_COLUMNS, |c| c.icontains(q))`.
- **Filters:** a `search` filter keeps the SQL it generated before, but builds it as one `rok_data` condition (`Expr::raw` with bound values) that the model's chain filters by. Soft deletes come from the model's scope (`include_deleted` is `with_trashed()`). The runtime's `where_has` was the alternative, but it can't express what the relationship filters already handle: a table related to itself (filters alias each nesting level), a many-to-many through its join table, and a related tenant table correlated on the tenant.
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
- `db/search.rs` keeps the filter's condition types and `Sort`/`Direction`. `Window` and `Limit` go once nothing generated pages with them.
- The runtime gains `Column::icontains`, `Raw::scalar_optional`, `Raw::scalars`, and `&[T]` binds for enum and typed-id lists.
- The milestone snapshots and the templates' generated files refresh through `ROK_DATA_SQLC_UPDATE_EXPECTED=1`.
