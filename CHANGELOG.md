# Changelog

All notable changes to the `rok-data` workspace are documented here. The
format is [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and
the version numbers follow [Semantic
Versioning](https://semver.org/spec/v2.0.0.html). Release notes for a
tag are this file's `## [X.Y.Z]` section (the release workflow checks
that it exists and won't ship a tag without it).

## [Unreleased]

**Changed**

- The actix and axum templates moved from `crates/rok-data-sqlc/templates/`
  to a crate of their own, `rok-templates` (workspace ADR 0022), which
  `rok-data-sqlc` depends on. `rok sqlc init --template` copies the same
  files as before. The engine's plain-init scaffold moved from
  `src/init/templates/` to `src/init/scaffold/`.
- `rok sqlc check-arch` treats `src/app/` as `app.rs` split up: its files
  may use the framework and `sqlx` as `app.rs` does, and it is no longer
  an unknown top-level directory. Rules 1 and 2 now read "`app.rs` or
  `app/`"; regenerate a project's `AGENTS.md` rules section from
  `rok sqlc init` output if you keep it in step.
- Both templates' wiring is split by responsibility into `src/app/`:
  `state.rs`, `router.rs`, `run.rs` and `ports.rs` (the last shared by
  both templates). The public paths (`app::AppState`, `app::run`, …) are
  unchanged.
- The templates' `Config::from_env` reports every wrong setting at once
  (`ConfigErrors`) instead of stopping at the first, and the server logs
  its settings at start-up with secrets left out (`Config::summary`).
  The config unit tests moved to `tests/config.rs`.

**Added**

- The templates' security baseline (`src/app/middleware.rs` over the
  shared `src/shared/security/headers.rs`): security headers on every
  answer (`nosniff`, `X-Frame-Options`, `frame-ancestors 'none'`,
  `Referrer-Policy`, and HSTS when `APP_URL` is `https://`);
  `CORS_ALLOWED_ORIGINS` (`*` or a list; unset, the API's own origin
  only); a panicking handler answered `500 internal_error` instead of a
  dropped connection; `GLOBAL_RATE_LIMIT`, a per-IP limit on every route
  but health (off by default); `JWT_SECRET_PREVIOUS`, to rotate
  `JWT_SECRET` without signing anyone out; and `deny.toml`, a
  `cargo deny check` policy that both templates pass. It ignores eight
  advisories no update can clear yet, each with the reason it can't reach
  the template (HS256-only JWTs, a build-time `zip`, the configured S3
  store, actix's TLS-only HTTP/2). Both templates' `Cargo.lock` files were
  refreshed: complete, and on `tokio` 1.54.1 (the yanked 1.54.0 had an
  advisory).
- The actix template's application is `app::app(state)`, the routes of
  `app::configure` wrapped in that middleware; tests build it with
  `test::init_service(app(state))`. It depends on `actix-cors` and
  `futures-util` now.

## [0.4.0] — typed keys, a runtime-backed data layer, and direct uploads

Breaking. Model lookups take the model's own key type, and the code
`rok sqlc` generates calls the runtime instead of sqlx. Follow
`docs/migration-guide.md` §To 0.4.0: regenerate, then fix what the
compiler reports.

**What changed**

- **Lookups take the model's own key type** (runtime). `Model` gains
  `type Key`, set by `#[derive(Model)]`:
  - for a single-column key, the key field's type;
  - for a composite key, a tuple of the key fields' types, in field order;
  - for a table without a key, `()`.

  `find`, `find_or_fail`, `find_many`, the `ModelCollection` key lookups
  and `audit::history` take `impl Into<Self::Key>`, so
  `Product::find(&db, category_id)` no longer compiles. A composite key of
  the wrong length used to fail at run time (`InvalidQuery`); now it fails
  to compile.
  - **Breaking for a hand-written `impl Model`:** add a `type Key` line.
- **Generated data calls go through the runtime** (engine ADR 0023).
  - **Errors:** generated functions fail with `rok_data::Error` instead of
    `sqlx::Error`. Annotated queries run through `rok_data::raw`.
  - **Reads:** generated reads are the runtime's verbs (`Row::find`,
    `Row::all`, `Row::count`) plus chains built from the schema:
    `Row::by_primary_key(…)` and `Row::by_<columns>(…)`, which replaces
    `get_by_<columns>`.
  - **Lists and search:** `Row::matching(…)` and `Row::search(filter)`
    return a chain that the caller orders and pages, so a page and its total
    take one round trip.
  - **Removed:** `get_by_id`, `exists_by_id`, `list`, `list_page`, `count`,
    `search_after` and the deprecated `write::` wrappers.
  - **Composite key structs** convert into the model's key tuple.
  - **Still on sqlx:** two generated paths, the versioned `patch` and the
    relationship loaders. A later release converts them.

  The runtime gains `Raw::scalar_optional`, `Raw::scalars`, `&[T]` binds
  for enum and typed-id lists, `NaiveDateTime`/`NaiveTime` list binds, and
  `Column::icontains`. The engine's CHANGELOG lists every changed
  generated item.
- **Direct uploads in both templates** (engine ADR 0024).
  - **Uploads:** `POST /media/uploads` answers a presigned `PUT` URL.
    `POST /media/{id}/complete` then checks the stored bytes' size and
    detected type.
  - **Downloads:** `GET /media/{id}/url` answers a presigned download URL.
  - **Signing:** `s3://` stores sign the URLs themselves (SigV4).
    `file://` and `memory://` URLs are signed by the API, under a key
    derived from `JWT_SECRET`.
  - **Cleanup:** a background sweep deletes uploads that are never
    completed.
  - **Breaking for a project made from a template:** migration
    `0006_media_uploads` is new, and so are the settings
    `MEDIA_UPLOAD_URL_TTL_SECS`, `MEDIA_DOWNLOAD_URL_TTL_SECS`,
    `MEDIA_UPLOAD_SWEEP_SECS` and `STORAGE_PUBLIC_ENDPOINT`.
    `shared/storage/` is scaffolded once, so an existing project copies the
    change in by hand.

**Added**

- **`rok sqlc check --db`** (engine ADR 0026). It prepares every annotated
  query against the database without running it, and reports each type,
  name or nullability mismatch with the generated code as
  `file:line: Query: message`. Without `--db`, `rok sqlc check` fails on
  drift, as `generate --check` does.
- **`rok sqlc generate --watch`** regenerates whenever the schema, the query
  files or the config file change.
- **`rok sqlc query add <NAME> <SQL> --file <stem>`** appends one annotated
  query and generates. The file keeps the query only if it generates.
- **`rok_data::introspect::describe(db, sql)`** answers a statement's
  parameter and column types without running it.
- **`db::prelude`** brings in the generated search vocabulary too, so one
  `use` serves a handler.

## [0.3.0] — the model-shaped data layer

Breaking. The data layer's surface becomes the model's: the typed verbs
live on the model, not in generated module namespaces, and hand-written
models speak the same dialect the generated ones do
(`docs/specs/0.3.0-model-shaped-data-layer.md`, workspace ADR 0014 /
engine ADR 0018). Follow `docs/migration-guide.md` §To 0.3.0; the
compiler finds the call sites.

**What changed**

- **The actix template has accounts** (rok-sqlc 0.6, absorbed; engine ADR
  0020). `rok sqlc init --template actix` now makes an API with sign-up,
  sign-in, email verification, password reset and change, email change
  and invitations. These are hand-written features, with a mail outbox,
  a `/auth` rate limit and a per-request account check.
  - **Breaking for a project made from the template:** `JWT_SECRET` must
    be at least 32 bytes, there is a new schema file and migration
    (`0002_auth`), and `app::configure` takes the `AppState`.
  - **Rule:** `rok sqlc check-arch` gains `cross-feature-query`.
  - **The axum template** has the same accounts (rok-sqlc 0.7). Its
    framework-independent files are byte-identical to actix's, which
    `template_drift` checks. `rok init -w axum` (and `rok sqlc init -w
    axum`) scaffolds the skeleton for axum.
  - **Both templates gain two-factor sign-in, API keys, uploads and
    profile photos** (rok-sqlc 0.8, engine ADR 0022). New required
    setting `TOTP_ENCRYPTION_KEY`; uploads are stored where `STORAGE_URL`
    says (`file://`, `s3://`, `memory://`); a disallowed upload type
    answers `415 unsupported_media_type`. Three new migrations
    (`0003_two_factor`, `0004_api_keys`, `0005_avatars`), and new
    dependencies in the templates' own manifests only. See the engine's
    [Files and storage](crates/rok-data-sqlc/docs/guide/files.md).
  - **Guide:** see the engine's
    [Accounts and sign-in](crates/rok-data-sqlc/docs/guide/auth.md).
- **Generated writes answer a missing reference or a taken value as a
  field's `422`** (rok-sqlc 0.5, absorbed; engine ADR 0019). Before, they
  answered a `409 conflict` naming a constraint. A create or update checks
  `exists` (each foreign key names a row the caller can see) and `unique`
  (no other row holds a unique key's values, ignoring case for a
  `lower(<column>)` index) before it writes. The `409` stays for races.
  `[api.rules]` adds string rules by VineJS name (`email`, `url`,
  `minLength(3)`, `trim`, …). See the engine's CHANGELOG.
- **Generated lists can page by cursor** (dx-first ticket 16):
  `[api] cursor = ["events"]` makes a table's `GET` list take
  `?cursor=`. Each page answers the cursors of the pages on either side,
  in the `pagination` shape (`{items, next, prev, per_page}` when
  compact). Pages stay stable while rows are added. A cursor the list
  didn't give out is a `400`. The run refuses a listed table without a
  primary key, or with a nullable sort key.
- **Keyset cursors carry typed keys**: a sort key or primary key whose
  type is a newtype over a built-in one (the generated `BookId`, any
  `impl_value!` / `DbNewtype` over `BIGINT`, `UUID`, `TEXT`, …) now
  travels in a cursor as its plain value. Before, the cursor of a
  generated model's page failed to parse, so `cursor_paginate` couldn't
  go past the first page.
- **Generated list endpoints answer in the page shape you pick**
  (dx-first ticket 15): `[api] pagination = "compact"` (the default,
  unchanged `{items, page, per_page, total}`), `"paginator"` (the flat
  paginator object with `current_page`, `last_page`, `from`/`to` and a URL
  per neighbouring page) or `"resource"` (`{data, links, meta}`). The
  links come from the request's URI, so every other query parameter is
  kept; the OpenAPI document describes the chosen shape.
- **The write verbs live on the generated model** (dx-first ticket 03,
  as 0.3.0 spec D5 and engine ADR 0018 promised): `Book::create(&db,
  input)`, `Book::patch(&db, id, patch)`, `Book::destroy(&db, id)`, and
  `restore` / `delete_permanently` / `attach` / `detach` / `sync` where
  they apply. The module functions (`books::write::create`) remain for one
  release as `#[deprecated]` wrappers that forward to the model verb.
  Generated services call the model verbs. A `[generate.scopes]` entry
  named like one of these verbs is rejected (it would collide).
- **Generated code reads like hand-written code** (dx-first ticket 03):
  chains start at the model (`Book::filter(Book::ID.eq(id))` instead of
  `Book::query().filter(…)`), and a row carries only the attributes the
  derive cannot infer (0.3.0 spec D4) — no `#[rok(table = …)]` when the
  table is the struct's pluralized snake_case name, no
  `#[rok(primary_key)]` on a lone `id` key. Schema-qualified tables and
  other keys keep theirs.
- **Every documented sample compiles** (dx-first ticket 14): the Rust
  samples of `docs/getting-started.md`, `docs/migration-guide.md` and the
  new `docs/pagination.md` each name the compiled file that holds them
  (`<!-- source: … -->`), and a test fails when one drifts; a migration's
  0.2 "before" says why it is not compiled. Fixing them fixed the samples
  that did not compile (a relation called on an `Option`, a scope on a
  model without it, placeholder scope paths). The generated
  `db::prelude` now also brings each model's inputs, ids and keys
  (`NewBook`, `UpdateBook`, `BookId`), and the runtime prelude brings
  `Cursor` and `CursorPage`.
- **Pages on the wire** (dx-first ticket 13, the runtime half): with the
  `serde` feature, `Page`, `SimplePage` and `CursorPage` answer as the
  paginator object (`page.paginator(links)`: `data`, `current_page`,
  `last_page`, `from`, `to`, the `*_page_url`s, …) or in the resource
  envelope (`page.resource(links)`: `{data, links, meta}`),
  with `rok_data::wire::Links::from_request(uri)` building every link from
  the request — the other query parameters kept, the page parameter
  nameable (`page_param`), an optional `fragment`. Each type's own
  `Serialize` stays the compact shape. `CursorPage` carries its
  `per_page`. Generated endpoints choosing a shape and generated cursor
  lists follow (tickets 15 and 16).
- **Walking a large table** (dx-first ticket 12): `chunk` reads one
  `LIMIT`/`OFFSET` window per query (ordered by the primary key when the
  query has no order) instead of one streamed query, so no connection is
  held while the callback runs — a callback that writes back through the
  same pool works with a pool of one, where it used to time out. New:
  `chunk_by_id` (keyset windows by primary key, correct when the callback
  deletes or updates its rows), `lazy` / `lazy_by_id` / `lazy_by_id_desc`
  (a `Stream` of rows read in windows), `each` / `each_by_id` (one row per
  call). A callback returns `Ok(())` to go on, or `Ok(false)` /
  `ControlFlow::Break` to stop (the `Walk` trait). Every walk takes a
  session — the database, a pool, a transaction or a connection.
- **Cursor pagination in both directions** (dx-first ticket 11): a
  `CursorPage` carries `prev` beside `next` (`prev_cursor()`,
  `next_cursor()`, `has_prev()`, `has_more()`), and passing a page's
  `prev` to `cursor_paginate` returns the page before it, rows in the
  query's order — for plain column orders and for the wrapped keyset of
  joins and expression orders alike. A cursor records its direction
  (`Cursor::before`, `is_backward()`); a backward token starts with `-`,
  and forward tokens are byte-for-byte those of 0.2. The JSON of a
  `CursorPage` gains `"prev"`. The size argument is named `per_page`.
- **Offset pagination, complete** (dx-first ticket 10):
  `simple_paginate(&db, page, per_page)` returns a `SimplePage` (the rows
  and `has_more`, one query, no `COUNT(*)`); `page(&db, n)` paginates at
  the model's size — `#[rok(per_page = 25)]`, 15 by default
  (`Model::PER_PAGE`); `for_page(page, per_page)` is the window as chain
  sugar; `.with(..).paginate(..)` returns a `LoadedPage` (the page's
  numbers beside the loaded includes); relation chains
  (`user.posts().paginate(..)`) paginate like any chain. `Page` gains
  `last_page`, `first_item` / `last_item`, `on_first_page` /
  `on_last_page`, `has_pages` and `into_collection`.
- **One pagination vocabulary** (dx-first ticket 09): "page" means a
  page of rows. The generated limit/offset type `db::search::Page` is now
  `db::search::Window` (`Page` stays a deprecated alias for one release),
  and a runtime `rok_data::Page<T>` converts to the wire's
  `PaginatedResponse<T>` in one call — a hand-written handler can answer
  `PaginatedResponse::from(Book::query().paginate(&db, page, per_page)
  .await?)`. The wire JSON and OpenAPI schemas are unchanged.
- **Write API polish** (dx-first ticket 08): `update_only` takes any list
  or iterator of columns — `user.update_only(&db, [User::NAME])`, the
  shape `upsert_on` already takes, instead of `Some(vec![…])` — and an
  empty list is an `Error::InvalidQuery`, never a silent full `update`.
  The hidden `delete_by_key` is no longer a trait method callable on every
  model.
- **Model collections**: `Select::get` returns a `ModelCollection<M>`
  (`all` keeps the plain `Vec`), so `User::query().get(&db).await?.find(1)`
  needs no conversion. Through `Deref` it is a `Vec`; its verbs work by
  primary key — `find`/`find_or_fail`, `contains_key`, `model_keys` (each
  row's whole key, composite keys included), `unique`, `only`/`except`,
  `intersect`/`diff`, `partition` — and go back to the database:
  `to_query`, `fresh`, `load`/`load_missing`. It serializes as the array of
  its rows (`to_json`, `Serialize`); a JSON failure is `Error::Json`
  (`is_json()`). Build one from a `Vec`, an array or an iterator (0.4.0
  ticket 01, dx-first ticket 07).
- **The API layer keeps its 0.2 names** (dx-first ticket 05): the
  generated handlers and service methods (`update`, `remove`,
  `remove_permanently`, `Remove{Row}Query`), the OpenAPI `operationId`s,
  the `[api.skip]` names and `ApiEndpoint::Remove` are those of 0.2.0;
  only the data layer's verbs take the new names. A `service.rs` override
  and an OpenAPI client keyed on `operationId` keep working.
- **One "not found" convention** (dx-first ticket 04): every verb that
  targets one row by key fails with `Error::NotFound` when no row has it.
  The runtime adds `Update::returning_one_or_fail` and makes
  `Error::not_found::<M>(key)` public; generated services answer the
  same 404 as before through a generated `or_not_found` helper.
- **One import**: the generated `db/mod.rs` writes a `prelude` module.
  `use crate::db::prelude::*;` brings rok-data's prelude, every model,
  and each model's `{Model}Scopes` trait, so chained scopes no longer
  need a hand-written `use …::models::UserScopes` (dx-first ticket 02).
- **The write spine renaming**: the generated write
  verbs are `create` / `patch` / `destroy`; `patch` returns the patched row
  (`Result<User>`), `destroy` returns `Result<()>`, a missing key is
  `Error::NotFound` (dx-first ticket 04), and the soft-delete pair keeps `restore` / `delete_permanently`
  (ticket 01). `save` is removed, not aliased (its meaning is ambiguous);
  the row-level verbs are `insert`, `update`, `update_only`, `upsert`,
  `upsert_on`, and these move to a separate `ModelWrite` trait — `Model`
  is the read surface, `ModelWrite` the write surface, both in the
  prelude so `use rok_data::prelude::*;` keeps working (ticket 04).
- **Conventions are inferred, not declared**: a `created_at`
  /`updated_at` pair manages its timestamps without
  `#[rok(timestamps)]`, a `deleted_at` field soft-deletes by default, a
  single-column primary key is inferred from the id field and generated
  `id`s skip the create body — each with an explicit `no_*` opt-out when
  a model genuinely does not want the inference.
- **Relations on the model** (ticket 02): the schema's foreign keys
  become `#[rok(belongs_to = …)]` / `#[rok(has_many(posts = Post::AUTHOR_ID))]`
  on the generated rows, expanded by the derive into typed relation
  constants and accessors — `user.posts()` is a lazy chain, and
  `db::relationships::load_posts` stays the batched loader.
- **Named scopes** (ticket 03): `#[rok(scopes(popular = path::to::popular))]`
  gives the model a chainable entry (`User::popular()`) and the chain a
  same-named method (`User::popular().active()`), through a `UserScopes`
  trait the derive writes next to the model. A scope takes the chain by
  value and returns it (`fn popular(q: Select<User>) -> Select<User> {
  q.filter(…) }`, dx-first ticket 06); a scope named like a runtime verb
  fails the derive (a test keeps that list in step with the runtime). A
  scope may take parameters declared in the attribute
  (`#[rok(scopes(older_than(n: i32) = …))]`), and generated models get
  their scopes wired from the engine's new `[scopes]` configuration
  (one `#[rok(scopes(name = path))]` attribute per scope; the app owns
  the functions). `default_scope` and `.unscoped()` are unchanged.
- **Chain additions and spellings**: `Model::exists`, `Model::where_` /
  `Select::where_` (another name for `filter`; the raw
  identifier is the only form that compiles since `where` is a keyword),
  `Select::get` (another name for `all`), and `Select::chunk`
  for walking big tables in windows.
- **Chain sugar** (0.4.0 ticket 02), every condition typed on its column:
  - **Several columns, one condition:** `filter_any([User::NAME,
    User::EMAIL], |c| c.ilike(&pattern))`, `filter_all`, `filter_none`.
  - **Date parts:** `date()`, `time()`, `year()`, `month()`, `day()`,
    `hour()` on a column, compared like any value
    (`Post::PUBLISHED_AT.year().eq(2026)`).
  - **JSON:** `json_array_len()`, `json_array_len_at(path)`,
    `json_has_path(path)`, `json_contains_at(path, json)`.
  - **Two columns of a row:** `eq_column` … `lte_column`
    (`Task::UPDATED_AT.gt_column(Task::CREATED_AT)`); `NULL`-aware
    equality with `is_not_distinct_from` / `is_distinct_from`.
  - **Raw escapes:** `filter_raw`, `having_raw`, `order_by_raw`,
    `group_by_raw`, `select_raw`, each `?` bound in order.
  - **Composing:** `pipe(f)` hands the chain to a function and returns
    its result; `tap(f)` looks at it and goes on; `debug()` prints the
    statement and its parameters to stderr.
  - **`UNION`:** `select.union(other)` / `union_all(other)` return a
    `Union` with its own `order_by`, `limit`, `offset` and `all` / `get` /
    `first`.
  - **Counters:** `Update::increment_each([(Post::VIEWS, 1), (Post::SCORE,
    5)])`, in one `UPDATE`.
- **Rows by their relations** (0.4.0 ticket 03, first half):
  - **Filters:** `has(User::POSTS)`, `doesnt_have(…)`, `where_has(rel, |q|
    q.filter(…))`, `where_doesnt_have(…)` and `where_relation(rel,
    condition)`, as correlated `EXISTS` subqueries that keep the related
    model's scopes.
  - **Aggregates beside the rows:** `with_count(User::POSTS)` and
    `with_exists(User::PROFILE)` on a chain, `load_count` on a collection,
    read with `view.count(User::POSTS, user)` / `view.exists(…)`. Sums,
    averages, minimums and maximums are handles on the relation
    (`User::POSTS.sum::<i64>(Post::VIEWS)`, `avg`, `min`, `max`), loaded
    with `with` and read with `view.aggregate(handle, user)`. Each is one
    batched `GROUP BY` query; the value type picks the SQL cast.
  - **Through relations:** `#[rok(has_many_through(posts = Post::USER_ID,
    through = User::COUNTRY_ID))]` on `Country` gives `Country::POSTS` (a
    `HasManyThrough`) and `country.posts()`; `has_one_through(…)` the
    one-target form. They load with `with`, filter with `has` /
    `where_has`, and count with `with_count` like a direct relation; the
    intermediate model's scopes apply. Declared, not inferred from the
    schema.
  - **Many-to-many:** `#[rok(belongs_to_many(roles = Role, pivot =
    "role_user"))]` gives `User::ROLES` (a `BelongsToMany`; pivot columns
    default to `user_id` / `role_id`) and `user.roles()`, loading,
    filtering and counting like the others. It writes the pivot:
    `attach` / `attach_with` (extra pivot columns), `detach` /
    `detach_all`, `sync`, `sync_without_detaching`, `toggle` and
    `update_existing_pivot`; `sync` and `toggle` are one statement each.
    The generator writes the attribute on both sides of a join table that
    pairs two primary keys (generated models change: a `belongs_to_many`
    line and an import).
- **Scopes and loaded views** (0.4.0 ticket 04): `#[rok(default_scope =
  …)]` may repeat — every one applies, combined with `AND`, and
  `.unscoped()` drops them together. `Loaded::get(rel, row)` reads a
  relation like `try_get` but fails with the new `Error::NotLoaded`
  (`is_not_loaded()`) when the query didn't load it, its message naming
  `.with(…)`.
- **Row lifecycle verbs** (0.4.0 ticket 08), on `ModelWrite`:
  `row.first_or_insert(&db, [Tag::SLUG])` returns the row holding that
  unique key, inserting this one first when there is none, in one atomic
  statement; `row.increment(&db, Post::VIEWS, 1)`, `decrement` and
  `touch(&db)` update in the database without reading first and return
  the stored row. `update_or_create` is the existing `upsert_on`, `fresh`
  the existing `reload`.
- **Input validation** (input-validation ticket 02, workspace ADR 0021):
  `#[derive(Input)]` on any struct gives `input.validate(&db).await?`
  (and `validate_fields()` without a session), returning the input
  transformed or every failure as `Error::Validation`. Fields take the
  model's `#[rok(validate(…))]` rules and `#[rok(transform(trim,
  lowercase, uppercase, normalize_email, custom = path))]`, run before the
  rules; `#[rok(validate_with = path)]` checks the whole input. A
  transform on a model, or a key / timestamp / relation attribute on an
  input, fails to compile.
- **The rule catalogue in the runtime** (input-validation ticket 07,
  first part): `validate(url, uuid(versions = [4]), ulid, ip(version =
  4), alpha(spaces, dashes, underscores), alpha_numeric(…), ascii,
  one_of(values = [...]), not_one_of(…), starts_with = "…", ends_with =
  "…", hex_code, iban, jwt, credit_card(providers = [...]), coordinates)`
  on models and inputs, each answering the generated APIs' code
  (`alphaNumeric`, `ipAddress`, `in`, …) and message. The checks live in
  `validate::catalogue`, moved from the engine's generated
  `shared/validation.rs`. The `normalize_email` transform now normalizes
  as the generated APIs' `normalizeEmail` does (provider subaddresses,
  Gmail dots) and no longer trims: add `trim` before it.
- **Validation context** (input-validation ticket 08, first part):
  `#[rok(context = Caller)]` on an input lets its database rules read
  who is asking: `scope = fn(&Self, &Caller) -> Expr<…>`, custom rules
  `async fn(&T, Journey, &Caller)`, `validate_with_database` likewise;
  validate it with `input.validate_with(&db, &caller)` (`validate(&db)`
  stays for inputs without a context). `Input` gains `type Context`
  (`()` unless declared); nested inputs share their parent's.
- **Generated APIs answer with the runtime's validation errors**
  (input-validation ticket 08, second part). The generated
  `ValidatedJson`, `FieldFailures` and the scaffolded `AppError` carry
  `rok_data::ValidationErrors`, so a generated `422` and a hand-written
  `input.validate(&db)` answer the same body. Breaking on the wire: a
  generated failure's `params` no longer echo the rejected `value`.
  `rok_data::Error::Validation` now maps to a `422` with its `details`.
  The runtime's `serde` support is always on (the `serde` feature stays,
  empty, for compatibility). A project made from a template keeps
  compiling once its `error.rs` imports `rok_data::ValidationErrors` in
  place of `validator::ValidationErrors` (migration guide).
- **Generated validation is the runtime's `Input`** (input-validation
  ticket 08, third part; engine ADR 0025). Breaking for generated code and
  the templates:
  - **Derives:** `derive = ["validate"]` puts `rok_data::Input` on `NewX`
    and `UpdateX` in place of `validator::Validate`. The API's bodies
    derive it too.
  - **Rules:** a body's rules read `#[rok(transform(trim),
    validate(alpha_numeric(dashes), min_length = 3))]`, the runtime
    catalogue's names with VineJS codes. `dto.rs` has no generated check
    functions.
  - **Extractor:** `ValidatedJson<T>` takes any `T: rok_data::Input`
    without a context.
  - **Dependencies:** an app drops `validator` (and `regex`), and enables
    rok-data's `regex` feature for pattern rules.
  - **On the wire:** an exact length answers `params` `{"min": n, "max":
    n}`.
  - **`custom` on an `Option`:** a `custom` rule on an `Option` field now
    runs only on `Some`, with the value inside, as the other rules do.
- **Exclusive ranges and decimal ranges** (input-validation ticket 08):
  `range(exclusive_min = 0, max = 5)` leaves a bound out (`"must be
  greater than 0 and at most 5"`, JSON Schema `exclusiveMinimum`); a
  `Decimal` field takes `range` (feature `decimal`); an integer bound
  stays an integer in the rule's `params`. New: `validate::Bound`,
  `validate::bounded`.
- **Custom codes and patch transforms** (input-validation ticket 08):
  `custom(function = path, code = "immutable")` answers its own code
  (`custom = path` stays code `custom`); transforms apply through an
  `Option<Option<String>>` patch field.
- **The rest of the catalogue** (input-validation ticket 07, second
  part): `min_length = n`, `max_length = n`, `fixed_length = n`
  (characters, codes `minLength` / `maxLength` / `fixedLength`), and with
  the new runtime feature `regex`: `mobile(locales = [...], strict)`,
  `postal_code(countries = [...])` and `regex = "…"`. Their pattern
  tables live in `validate::patterns` and in the engine; a test keeps the
  two equal. Generated APIs now import the rule checks
  (`is_url`, `letters`, `normalize_email`, …) from
  `rok_data::validate::catalogue` instead of carrying copies in
  `shared/validation.rs`.
- **JSON Schema from an input** (input-validation ticket 06):
  `Input::json_schema()` (feature `json`) is the input as JSON Schema
  2020-12 — field types, `required`, and the field rules a schema can say
  (`minLength`/`maxLength` or `minItems`/`maxItems`, `minimum`/`maximum`,
  `format: email`); nested inputs inline, enum inputs as `oneOf`; a type
  it can't describe is `{}`. The mapping is documented on
  `validate::schema`.
- **Messages and the reporter** (input-validation ticket 05):
  - **Messages:** every rule takes `message = "{field} needs {min}+
    characters"` (`email(message = "…")` for the bare ones), quoting
    `{field}` and the rule's arguments. A `Messages` provider (rule code
    and field → template) applies through `errors.localized(&provider)`,
    leaving a rule's own message alone.
  - **Errors:** a `FieldError` carries its rule's arguments as
    `params: Vec<(&str, Value)>`.
  - **Reporter:** `ErrorReporter` turns errors into a body;
    `DetailsReporter` writes the generated APIs' `{"error": {"code":
    "validation_failed", "message", "details": {path: [{code, message,
    params}]}}}`.
  - **Breaking:** the `axum` responses of every `rok_data::Error` use the
    wire standard's `{"error": {"code", "message", "details"?}}` shape
    instead of `{"error": code, "message", "fields"?}`.
- **Input shapes** (input-validation ticket 04): `required_if(field =
  kind, equals = "company")`, `required_unless(…)` and
  `required_with(other)` (code `required`, presence by the `Presence`
  trait: `Some`, non-empty text or list); `#[rok(when = path)]` gates a
  field's rules; `#[rok(nested)]` validates an input field, an `Option`
  or a `Vec` of them with the outer one — transforms, rules and built-in
  database rules — at paths like `address.city` / `lines.2.sku`; an enum
  whose variants each wrap an input delegates to its variant.
- **Database rules on input** (input-validation ticket 03):
  `unique(User::EMAIL, except = id, case_insensitive, scope = path)` and
  `exists(Org::ID, scope = path)` in an input field's `validate(…)`, the
  target model's own scopes applying; `database = path` on a field and
  `#[rok(validate_with_database = path)]` on the input for checks of
  their own. `validate(&session)` runs them only on fields whose field
  rules passed, the built-in ones in one statement. On a
  `#[derive(Model)]`, a database rule fails to compile. New:
  `Column::eq_ignore_case`, `Journey::reborrow` (public).
- **Validation errors carry paths; violations read as field errors**
  (input-validation ticket 01, workspace ADR 0021). Breaking:
  `FieldError::field` is a `String` path (`address.city`, `items.2.sku`)
  and `FieldError` gains `params`. A unique or foreign-key violation on a
  row write is `Error::Validation` at the column, code `unique` /
  `exists`, instead of `Error::Database`; `is_unique_violation()` /
  `is_foreign_key_violation()` stay true. See the migration guide.
- **What a field type can't say** (0.4.0 ticket 05, narrowed by
  decision: a field's Rust type already is its cast):
  - **Encrypted columns:** `Encrypted` (feature `encryption`) stores
    `base64(nonce ‖ AES-256-GCM ciphertext)` in a `TEXT` column and gives
    the plain text back; the key comes from `set_encryption_key` or
    `ROK_DATA_ENCRYPTION_KEY`.
  - **Value objects:** `#[rok(value_object(address: Address = (street,
    city)))]` generates `address()` and `set_address(…)`.
  - **Write-time transforms:** `#[rok(set_with = hash)]` on a field
    generates `set_<field>(value)` storing `hash(value)`.
- **Nested eager loading** (0.4.0 ticket 10):
  `.with(Author::POSTS.with(Post::COMMENTS))` loads each level in one
  batched query, to any depth; `view.nested(Author::POSTS)` is the
  `Loaded<Post>` view the nested relations are read from (the nested
  target must be `Clone`). `Loaded::load(&db, includes)` loads (or
  reloads with other shaping) beside the existing `load_missing`. The
  relation-constant extension trait is `RelationExt` (aggregates and
  `with`).
- **`INSERT … SELECT`** (0.4.0 ticket 09): a projected query writes its
  rows into another table without fetching them —
  `Post::filter(…).select((Post::TITLE,)).insert_into::<Archive>([Archive::TITLE])`,
  then `exec` (the row count) or `returning` (the inserted rows).

**What does not change**: PostgreSQL 12+ (DATABASE_URL), the migration
system, `#[rok::test]`, the connection policy, `filter` staying the
canonical verb, and the two-lane principle (schema generates baseline
CRUD; query files carry application SQL).

## [0.2.0] — the unified ORM chains through the generated data layer

The front door grows two root commands: `rok init [NAME] [--template
<NAME>] [--schema <PATH>]` scaffolds a project — the application skeleton,
or a ready-made template — and seeds the project's schema directory with
the schema file it is given. Every file it writes is a scaffolded file:
written once, never rewritten.

`rok gen [OPTIONS] [check]` is the unified front door's generator: it
turns the project's schema directory into the data layer (`src/db/**`),
the shared tools and the API features (`src/features/**`), and — with
`--migration NAME` — stages the difference between the schema and the
last staged migration as a `NNNN_name.up.sql` / `.down.sql` pair in the
history directory. `rok gen check` is the CI mode: it writes nothing,
reports the missing, changed and stale files, and fails on drift. An
un-staged truth change is a finding of its own there — the prose names
the last staged migration and the version the pair would take
(`0002_pending`), and `--json` carries it in `pending_migration`; no
file that does not exist is reported among the missing ones, and the
run still fails. The settings file is the unified `rok.toml`
(workspace ADR 0011), whose `[generate]` section holds the generation
settings; the engine's `rok-data-sqlc.toml` stays readable. The legacy
`rok generate` and `rok sqlc …` keep their flags, their output and
their exit codes — the `sqlc` tokens are still forwarded verbatim to
the engine — and the root help now marks them as superseded by the
front door; their removal is deferred to 1.0. One spelling changed:
`rok generate` no longer takes the `generate` subcommand, so the write
mode is flat (`rok generate [OPTIONS]`) and the CI mode nests
(`rok generate check`).

The truth and the history now live in one directory each: `rok gen`
reads the truth directory — the plain `*.sql` files a hand writes,
`schema/` in the layout `rok init` seeds — and, with `--migration NAME`,
stages the full history into the history directory (`migrations` by
default): the `NNNN_name.up.sql` / `.down.sql` pairs and, hidden beside
them, the `.rok-schema.json` snapshot of the staged shape that the next
staging diffs against. `rok migrate` reads that history unchanged. Both
directories keep their pre-split default — one `migrations` directory
holds the truth and the pairs in an old project, and it keeps
generating and staging as before — and the split is wired in the
settings: the truth is named by `--schema` / `[generate] schema` as
before, the history by the new `[generate] migrations` key and `rok gen
--migrations-dir` (the flag beats the file). The loader that reads the
history skips hidden files, so the snapshot rides along unseen — and the
snapshot itself leaves the truth's `-- rok:down` undo statements out.
Where `--migration`
used to advise a `psql` invocation, it now points at the pair it staged:
`apply the staged pair (<up>) with `rok migrate up``, with `--dir`
added only when the history is not the default. `rok init` seeds the
truth (`schema/0001_init.sql`, whose `-- rok:down` lines mark its undo
SQL) and no history — the history appears with the first staged pair —
and the templates and [`examples/api`](examples/api) follow the same
layout: a `schema/` truth beside the pairs and the snapshot in
`migrations/`.

The generated per-table data layer (`models`, `read`, `write` and the
relationship loaders) now rides the `rok-data` model chains instead of
built SQL: the functions that ride the chains return `rok_data::Error`
where they used to return `sqlx::Error`. Joins, search/listing/filter and
`m2m` pair loaders stay on the Query builder for this slice, and still
return `sqlx::Error`. The actix template's skeleton gains the `rok-data`
dependency and an `AppError` conversion.

The generated API features have a second backend: `[api] framework =
"axum"` (or `-w/--api-framework axum`) generates them for axum instead of
actix-web. The routes are a `Router`'s, the handlers take axum's
extractors, and the shared tools (health, the tenant, the validated
bodies) are axum's. The bodies, the service and the OpenAPI document are
the same either way, and actix-web stays the default: every existing
project generates exactly what it did before. `rok sqlc check-arch`
follows this key as well: its rule 1 polices the imports of the API
backend the project is generated for (`actix_web` under the default,
`axum` under `framework = "axum"`), over the same allowed-file set either
way, so an axum project's architecture is checked like an actix one's
instead of passing rule 1 trivially.

`rok init --template axum` scaffolds the starter that matches it: the
`actix` template's twin, with the same `users` and `audit-log` features,
the same wire standard and the same contract test, written for axum. The
two templates differ only in the framework's edge — the routes, the
handlers, the extractors and the server setup — and both are checked in
CI: regenerated, linted, `check-arch`ed and tested.

The vertical slice of the pipeline is checked in as an example:
[`examples/api`](examples/api) holds a schema of two files, the generated
data layer and axum API features beside it, and the staged migration
pairs — one crate, from `rok init` and `rok gen` to a running API. It is a
crate of its own (workspace-excluded, like the templates and the
fixtures), and the repository's gates compile it: the drift check, the
staged pairs, rustfmt, the lint gate and its tests.

Also in 0.2.0: an unqualified `ALTER TABLE <name>` in a later
migration now reaches a table created under a schema
(`catalog.products`) when that SQL name is unique in the schema, and
diagnostics name the resolved table; a truth file can carry an
`ALTER TABLE` beside its `CREATE TABLE`s — the schema parser now skips
the `-- rok:down` undo statements the truth carries (previously the undo
was applied as a statement of its own and undid the truth's change), and
refuses a misplaced marker with the whole-crate generator's rule
wording; the `Model` derive accepts
no-primary-key tables; composite types (money, ranges, timetz,
inet/cidr, macaddr, bytea arrays) ride the new `ipnetwork` and
`mac_address` facade features.

The release is a tag: `vX.Y.Z` must equal the workspace's version, and
the tag publishes the `rok` binary for Linux (musl), macOS and Windows
(x86_64 and ARM64) as checksummed archives, then the six crates to
crates.io in dependency order (workspace ADR 0008). The repository's
`install.sh` and `install.ps1` fetch and verify those archives, so a
release installs with one line.

## [0.1.0] — initial consolidation

First release of the consolidated workspace. `rok-data` combines the
former `rok-db`, `rok-sqlc`, `rok-fluent` and `rok-orm` repositories
(their own histories and releases stay in their repositories; see
`docs/migration-guide.md` for every rename and its exact consequences).

### Added

- The `rok` CLI (`crates/rok-cli`), the ecosystem's single user-facing
  binary: `generate`, `generate check`, `migrate
  create|status|up|down`, `db check|inspect`, `schema dump|diff`, and
  the `rok-data-sqlc` engine behind `rok sqlc
  generate|init|check-arch`.
- `rok-data-core::{introspect, migration_file}`: catalog introspection
  (tables, columns, indexes, foreign keys, DDL dump) and the unified
  migration file source (file pairs plus single-file sections).
- A doc gate in CI: `cargo doc` over every crate with the same rustdoc
  strictness docs.rs builds with.

### Renamed (breaking)

- Crates, environment prefix `ROK_*`, journal, and config files keep
  their semantics but carry the new names: `rok-data*` crates,
  `rok-data.toml` and `rok-data-sqlc.toml`, journal
  `_rok_data_migrations`.
- Generated-file headers: `@generated by rok-sqlc` →
  `@generated by rok-data-sqlc` (both historical markers stay valid).
- MSRV unified to 1.94 (rok-sqlc previously supported 1.88; the
  workspace floor is the one line the generated code needs anyway).

### Removed

- The standalone binaries `rok-db-gen` and `rok-sqlc` (merged into
  `rok`, workspace ADR 0002).
- `rok-fluent` and `rok-orm` as separate runtimes (superseded; the
  absorbed pieces are documented in workspace ADR 0001).
