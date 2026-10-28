# Changelog

All notable changes to rok-data-sqlc are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Changes to
the *generated* API count as breaking (see ADR 0012).

## [Unreleased]

### Breaking changes to the generated API

Generated data calls move onto the runtime (engine ADR 0023):

- **Reads:** `read.rs` holds `impl Row` with chain-returning reads only:
  `by_primary_key(…)`, one `by_<columns>(…)` per unique key (was
  `get_by_<columns>`), and `search`. `get_by_id`, `exists_by_id`, `list`,
  `list_page` and `count` are gone. Use the runtime's verbs instead:
  - `get_by_id(&db, id)` becomes `Row::find(&db, id)`, or
    `Row::by_primary_key(tenant, id).first(&db)` on a tenant table;
  - `exists_by_id` becomes `Row::by_primary_key(id).exists(&db)`;
  - `list`, `list_page` and `count` become `Row::all`, `Row::query()…paginate(..)`
    and `Row::count`.

  A composite key struct implements `rok_data::IntoKey`, so `find` takes it.
- **Writes:** the deprecated 0.2 module wrappers (`<table>::write::create`
  and the rest) are gone; the verbs live on the model only.
- **API lookups:** `GET …/by-<column>/{value}` keeps its handler and service
  names (`get_by_<column>`) and reads through `Row::by_<column>`.
- **Owned reads:** `get_owned`, `lock_owned` and `exists_owned` are the
  row's `by_primary_key` chain scoped to the owner (`for_update()` for the
  lock), and fail with `rok_data::Error`.
- **Join-table writes:** `attach`, `detach` and `sync` run through
  `rok_data::raw` and fail with `rok_data::Error`.
- **Annotated queries:** they run through `rok_data::raw` and fail with
  `rok_data::Error` instead of `sqlx::Error`. They name the models they
  return bare (`Result<Option<Media>, rok_data::Error>`), imported once.
- **The list endpoint:** `list_matching` and `count_matching` become one
  `Row::matching(…, q, sort)` that returns a `rok_data::Select`. The
  service runs it with `.paginate(..)`, so a page and its total take one
  round trip. `q` matches through `Column::icontains`; the
  `push_contains_any` helper leaves `db/search.rs`.
- **Search:** `search(filter, sort, page)` and `search_after(filter,
  after, limit)` become one `Row::search(filter)` that returns a
  `rok_data::Select` (a `Result` when relationship filters can nest too
  deep). Order and page it on the chain: `.order_by(..)`,
  `.paginate(..)`, `.cursor_paginate(..)`.
- **Filters:** a filter builds one `rok_data` condition
  (`condition()`, internal) in place of `push_where` into a
  `sqlx::QueryBuilder`. Soft-deleted rows come from the model's scope;
  `include_deleted` is `with_trashed()`. Nesting too deep fails with
  `rok_data::Error::InvalidQuery`.
- **Sort keys:** a sort enum answers `order(direction)`, a typed
  `rok_data::Order`, and a `Sort` of it converts into one, so a chain
  takes it as is: `User::search(filter)?.order_by(sort)`. The data layer's
  `XSortKey::column()` is gone, and the API's sort enum gains both too.
- **`POST /search`:** `search_matching` and `count_search` become one
  `Row::search_sorted(…, filter, sort)` that the service pages.
- **`db/search.rs`:** `Window`, `Limit` and `MAX_PAGE_SIZE` are gone (the
  API's `Pagination` clamps the page size), and so is `From<Pagination>
  for Window`.
- **Service imports:** a service that never calls `repo::` no longer
  imports it.

### Added

- **`rok sqlc query add <NAME> <SQL>` (`q a`):** appends one annotated
  query to a query file (`--file media`, or a path) and generates. The file
  keeps the query only when it generates; otherwise it is restored and the
  error names the failure and its line. The return mode is inferred from
  the statement, or set with `-o`/`-m`/`-M`/`-x`/`-r`.
- **`db::prelude` brings the search vocabulary too:** each model's filter
  and sort key, `Sort`, `Direction`, `Equality`, `Ordered`, `Text`, and the
  generated enums (`types::*`). So `use crate::db::prelude::*;` is the only
  import a call site like this needs:

  ```rust
  User::search(filter)?.order_by(Sort::desc(UserSortKey::CreatedAt)).paginate(..)
  ```
- **Direct uploads in both templates (engine ADR 0024):** `POST
  /media/uploads` answers an upload ticket with a presigned `PUT` URL;
  `POST /media/{id}/complete` checks the stored bytes' size and detected
  type; `GET /media/{id}/url` answers a presigned download URL. `s3://`
  stores sign with SigV4 (`STORAGE_PUBLIC_ENDPOINT` for the address clients
  use); `file://` and `memory://` URLs are signed by the API, under a key
  derived from `JWT_SECRET`, and served at `/api/v1/storage/{token}`. A
  background sweep deletes uploads never completed. New settings:
  `MEDIA_UPLOAD_URL_TTL_SECS`, `MEDIA_DOWNLOAD_URL_TTL_SECS`,
  `MEDIA_UPLOAD_SWEEP_SECS`, `STORAGE_PUBLIC_ENDPOINT`. Migration `0006`
  adds `media.completed_at` and `expires_at`; existing rows count as
  completed. `shared/storage/` is scaffolded, so an existing project adopts
  the change by hand.

## Workspace 0.3.0 - 2026-10-28

Published as `rok-data-sqlc` 0.3.0 with the workspace (root `CHANGELOG.md`);
the version numbers below are the engine's own, from before it joined.

### Breaking changes to the generated API

- Generated validation is the runtime's `Input` (engine ADR 0025):
  - **Derives:** `derive = ["validate"]` derives `rok_data::Input` in place
    of `validator::Validate`. The create, update and search bodies, and
    `IdList`, derive it too.
  - **Rules:** a field's rules are `#[rok(transform(…), validate(…))]`, the
    runtime catalogue's. `dto.rs` has no per-column check functions, and
    `shared/validation.rs` no `check`, `matches_regex` or `Transformed`.
  - **Transforms:** they run before the rules, not inside
    `deserialize_with`.
  - **Extractor:** `ValidatedJson<T>` takes any `T: rok_data::Input`
    without a context and checks its field rules.
  - **Dependencies:** the data layer's hints drop `validator` and `regex`,
    and name rok-data's `regex` feature for `regex`, `mobile` and
    `postalCode`.
  - **On the wire:** an exact length's `params` are `{"min": n, "max": n}`,
    and no failure echoes the rejected `value`.

### Breaking changes to the templates

- From rok-sqlc 0.8.0 (absorbed; engine ADR 0022): both templates gain
  two-factor sign-in, API keys, uploads and profile photos, as
  hand-written features (`src/features/{two_factor,api_keys,media}/`, the
  photo routes in `account`). See [Accounts and sign-in](docs/guide/auth.md)
  and [Files and storage](docs/guide/files.md).
  - **Two-factor** (`/auth/2fa/*`): TOTP with ten recovery codes. Secrets
    are sealed with `TOTP_ENCRYPTION_KEY` (required: 32 bytes in base64)
    and bound to their account; recovery codes are HMACs under a key
    derived from it. Sign-in with two-factor on answers `{requires_totp,
    pending_token}`; disabling it signs every session out.
  - **API keys** (`/api-keys`): `read` or `write` scope, at most 25 active
    per account, only a hash kept. `AuthUser` accepts one as `Bearer
    rok_…` or `X-API-Key`; no key reaches `/auth` or `/api-keys`, however
    the path is encoded (`403`). `AuthUser` gains a `credential` field.
  - **Uploads** (`/media`) go through a `Storage` port opened from
    `STORAGE_URL` (`file://`, `s3://`, `memory://`): a raw body capped by
    `MEDIA_MAX_UPLOAD_BYTES` (`413`), its type detected from its bytes
    and checked against `MEDIA_ALLOWED_TYPES` (`415
    unsupported_media_type`, a new `AppError` variant). actix reads the
    body only after `AuthUser` succeeds.
  - **Profile photos:** `PUT`/`DELETE /users/me/avatar` and the public
    `GET /avatars/{file}`; `users.avatar_url` is a generated column.
  - **A password reset** revokes the account's API keys and spends its
    pending two-factor sign-ins.
  - **The schema:** new truth files `schema/0003_two_factor.sql` and
    `schema/0004_api_keys.sql`, and `users.totp_last_step` and
    `users.avatar_url` in `0001_init.sql`, staged as the migration pairs
    `0003_two_factor`, `0004_api_keys` and `0005_avatars`.
  - **New dependencies** (the templates' own manifests only): `aes-gcm`,
    `bytes`, `hmac`, `image`, `infer`, `object_store`, `qrcode`, `subtle`,
    `totp-rs`, `zeroize`.
  - **Breaking for a project made from an older template:** set
    `TOTP_ENCRYPTION_KEY`, apply the three migrations, and build the
    state with `AppState::new(pool, &config)?` (it opens the store and
    can fail).
- From rok-sqlc 0.7.0 (absorbed; engine ADR 0021): `rok sqlc init
  --template axum` makes the same API with accounts as the actix template.
  It has the same flows, services, DTOs, queries, migrations,
  `shared/security/`, `shared/mail/` and rate-limit counter, all
  byte-identical, which `template_drift` (a rok-cli test) checks.
  - **The generated routes stay stateless** (`features::router(pool)`,
    services as extensions).
  - **The hand-written `auth` and `account` routes** are a
    `Router<AppState>`.
  - **`AuthUser`** reads the JWT keys and the pool from the request's
    extensions, which `app::router` puts there, and checks the account on
    every request.
  - **The server** gets tracing, request ids, CORS, a 2 MiB body limit, a
    30-second timeout (`408 request_timeout`), graceful shutdown and the
    mail sender beside it.
  - **Breaking for a project made from the old axum template:** the same
    as actix's (`JWT_SECRET` of 32 bytes, `0002_auth`), and `app::app(&pool,
    &keys)` becomes `app::router(AppState::new(pool, &config))`.
- The actix template, from 0.7: passwords are hashed on tokio's blocking
  pool, the rate limiter moves to `shared/rate_limit/{mod,limiter}.rs`, and
  an unknown path or method under `/api/v1/auth` answers the wire
  standard's `404`.

- From rok-sqlc 0.6.0 (absorbed; engine ADR 0020): `rok sqlc init
  --template actix` makes an API with accounts.
  - **The flows:** sign-up, email verification, sign-in, password reset
    and change, email change and admin invitations, as hand-written
    features (`src/features/{auth,account}/`). They are backed by
    `src/shared/{security,mail}/`, the `/auth` rate limit
    (`src/shared/rate_limit.rs`) and per-feature SQL in `queries/`.
  - **The session check:** `AuthUser` checks the account against the
    database on every request (`token_version`, `is_active`, `role`), so it
    needs the pool.
  - **The secret:** `JWT_SECRET` must be at least 32 bytes, or the server
    doesn't start.
  - **New settings** in `.env.example`: `JWT_TTL`, `AUTH_RATE_LIMIT`,
    `TRUST_PROXY`, `BREACH_CHECK`, `REQUIRE_VERIFIED_EMAIL`, the mail
    settings, and `APP_URL`.
  - **The schema:** a new truth file, `schema/0002_auth.sql`
    (`auth_tokens`, `outbound_mail`), and `users.token_version`, staged as
    the migration pair `migrations/0002_auth`.
  - **Wiring:** `app::configure` takes the `AppState` that holds the
    outbox, the mail templates, the breach check, the rate limit and the
    audit port.

  A project made from an older template can copy the features; nothing
  generated changes for it. The axum template gains the same flows with
  the rok-sqlc 0.7 port.
- The actix template's config gains `[api.rules.users]`: an email is
  trimmed, lowercased and checked, and a display name is trimmed.

### Added

- `rok sqlc init -w/--api-framework axum` (and the same flag on `rok
  init`) scaffolds the skeleton for axum: `app::app(&pool, &keys)` over the
  generated `router(pool)`, axum's `AuthUser` and errors, and a config
  generating for axum. It doesn't combine with `--template`, which has its
  framework. From rok-sqlc 0.7.0's `init --framework`, spelled as
  `generate`'s flag.
- Under axum, a malformed path or query answers `400 bad_request` worded as
  actix-web words it (`Path deserialize error: can not parse "abc" to a
  i64`, `Query deserialize error: …`), so both frameworks answer the same.
  A path rejection that is a wiring bug is a `500`. From rok-sqlc 0.7.0.

### Fixed

- The data layer's `mod.rs` hinted `utoipa-swagger-ui` with the `actix-web`
  feature for every project; it now names the API's framework (`axum` for
  an axum project).

### Added (check-arch)

- `rok sqlc check-arch` reports the other framework's crate as a `rule-1`
  violation everywhere but the data layer, whose `extractors` may serve
  both. Example: `actix_web` in an axum project's `service.rs` (or `shared/`).
  Before, rule 1 ignored the crate the config doesn't name. From rok-sqlc
  0.7.0 (engine ADR 0021, A7).
- `rok sqlc check-arch` gains `cross-feature-query`: a feature calls its
  own query module, `<db_path>::queries::<feature>`, and never another
  feature's. A query module named after no feature is anyone's. From
  rok-sqlc 0.6.0.
- The guide gains [Accounts and sign-in](docs/guide/auth.md).

### Breaking changes to the generated API

- From rok-sqlc 0.5.0 (absorbed; engine ADR 0019): a generated create or
  update checks, on its transaction and before it writes, what the database
  would reject. The answer is a field's `422 validation_failed`, where it
  was a `409 conflict` naming a constraint:
  - **`exists`:** every foreign key of the body names a row the caller can
    see (live, in the tenant, theirs when the referenced table is owned).
  - **`unique`:** no other row holds a unique key's values (per tenant and
    owner, live rows only under a soft-delete partial index, ignoring case
    for a `lower(<column>)` entry).

  Every failure is listed in one answer, for example `{"slug": [{"code":
  "unique", ...}]}`. Clients that branched on the `409` see the `422`
  instead. The `409` remains for a race between the check and the write.
  A create or update whose body references an owned or tenant-scoped table
  may now take the caller or the `Tenant` its check needs, so that service
  method's signature can differ from the feature's other methods.
- A unique index with one `lower(<column>)` entry is read as a
  case-insensitive key: the table's `get_by_<column>` lookup compares
  ignoring case, and the data layer gains `<columns>_taken` and
  `exists_owned` reads in `listing.rs`.

### Added

- From rok-sqlc 0.8.0 (absorbed; engine ADR 0022, B3): schema reading
  follows `ALTER TABLE … DROP CONSTRAINT [IF EXISTS]`. A dropped unique
  constraint loses its lookup, and a dropped foreign key its relationship.
  A dropped check frees its column, so dropping a value-list check and
  adding a wider one widens the generated enum.
  - **Names:** unnamed checks and composite foreign keys get PostgreSQL's
    names.
  - **Untracked constraints:** dropping one the model doesn't track
    (`EXCLUDE`, PostgreSQL 18's `*_not_null`) now warns at its `path:line`
    and generates the table. Before, it failed the table.
  - **`pg_dump` checks:** `ADD CONSTRAINT … CHECK (col = ANY
    (ARRAY[…]))` is read as a value list.
  - **`RENAME CONSTRAINT`:** renames any constraint, and fails on an
    unknown old name or a taken new one.
  - **Parser warnings** now reach the run's output; before, they were
    collected and dropped.
- From rok-sqlc 0.5.0 (absorbed; workspace ADR 0016, engine ADR 0019):
  string rules for generated API bodies. `[api.rules.<table>]` maps a
  text column to rules and transforms by VineJS name (`email`, `url`,
  `uuid`, `mobile`, `alphaNumeric`, `{ regex = "..." }`, `minLength(3)`,
  `trim`, `toLowerCase`, `normalizeEmail`, …). The create and update
  bodies apply the transforms while the body is read, and check the rules.
  A failed rule is a `422 validation_failed` under its field, with the
  rule's name as the code. The OpenAPI document gains the `format`,
  `pattern` and lengths utoipa can carry. The run fails on an unknown
  rule, bad arguments, a `regex` that doesn't parse (checked with
  `regex-syntax`, a new engine dependency), and on the rules left out
  (`confirmed`, `sameAs`, `notSameAs`, `activeUrl`). Rules that match
  patterns make `db/mod.rs` name `regex = "1"`.
- `shared/validation.rs` gains `FieldFailures`, which collects a write's
  field failures into one `422`, for either framework. It is the seam the
  `exists` and `unique` pre-checks report through.

- The `axum` template (`templates/axum-api/`): the `actix` template's
  twin, generated for axum (`[api] framework = "axum"`), with the same
  `users` and `audit-log` features, the same wire standard, the same
  contract test against the axum starter's document, and a README. CI
  regenerates it and fails on drift, and runs its lint gate, `check-arch`
  and tests. `rok sqlc init --template axum` copies it.

## [0.4.0] - 2026-10-19

### Breaking changes to the generated API

- `extractors`: `db/http/` answers a rejected body with the wire standard
  of ADR 0016, `{"error": {"code", "message", "details"?}}`, on both axum
  and actix-web. The `success` field is gone and the codes are snake_case:
  `400` `bad_request` (was `MALFORMED_BODY`), `413` `payload_too_large`
  (was `PAYLOAD_TOO_LARGE`) and `422` `validation_failed` (was
  `VALIDATION_FAILED`). In `db::http`, `ErrorBody` loses `success`,
  `ErrorPayload` serializes `code` before `message`, the constant
  `MALFORMED_BODY` is now `BAD_REQUEST`, and `ErrorBody::new(code, message)`
  builds a body for any other error answer.

### Added

- `[api.immutable]`: columns set on create and never changed by the
  generated update, by table, in the config file and the JSON payload
  (`ApiConfig::immutable`). They stay in `CreateXDto` and the response,
  leave `UpdateXDto` and its OpenAPI schema, and a `PATCH` naming one
  answers `422 validation_failed` with the detail code `immutable`. The
  data layer's `UpdateX` keeps them. A key, owner, tenant or managed
  column fails the run. The actix template makes `users.email` immutable,
  so its contract no longer lists the update body's `email`.
- The Release workflow publishes the crate to crates.io once every binary
  builds, using the `CARGO_REGISTRY_TOKEN` repository secret. A version
  crates.io already has is skipped, so running a tag again doesn't fail.
- The `[api]` configuration for generated API features (ADR 0016):
  `output` (`-a/--api-output`), `db_path`, `prefix`, `search_endpoint`,
  `exclude`, and the table-keyed `[api.skip]`, `[api.routes]`,
  `[api.access]`, `[api.owner]`, `[api.hidden]`, `[api.search]` and
  `[api.sort]`, in the config file and the JSON payload. The library takes
  it as `Config::api` (`ApiConfig`, `ApiAccess`, `ApiEndpoint`), checked
  against the schema: an owner on a `public` table, or a search or sort
  column that is hidden, fails the run.
- With `api.output`, a run generates the infrastructure of ADR 0016:
  `db/pool.rs` (`PoolSettings`, `init_pool`, `check_health`) and, in
  `shared/` beside the features directory, `response.rs`
  (`PaginatedResponse` and the error body), `pagination.rs` (`ListQuery`,
  `Pagination`, `SortOrder`), `validation.rs`, `tenant.rs` (`Tenant<T>`) and
  `health.rs` (`/health/live`, `/health/ready`). Paths into the data
  layer follow `api.db_path`.
- API features (ADR 0016): with `api.output`, every table but join tables
  and `api.exclude`d ones gets `api.output/<module>/` with `mod.rs`,
  `routes.rs`, `handlers.rs`, `dto.rs` (`CreateXDto`, `UpdateXDto`,
  `XResponse`, `ListXsQuery`), `model.rs`, `repo.rs`, `service_api.rs` (the
  `XsServiceApi` trait with a default per endpoint, each write split into
  `<write>_in` on the caller's connection and `<write>` in a transaction)
  and a scaffolded `service.rs`; plus `generated.rs` (`register_services`,
  `configure_all`) and `README.md`. Endpoints: `GET /` (`page`, `per_page`,
  `q`, `sort`, `order`), `POST /` (`201` with `Location`), and `GET`,
  `PATCH` and `DELETE /{key}`; `[api.skip]` drops any. The data layer
  gains `<table>/listing.rs` (`XSort`, `list_matching`, `count_matching`)
  and `search::push_contains_any`; `shared/` gains `response::path_segment`
  and `validation::{path_config, query_config}`. Generated files under
  `api.output` and `shared/` are removed once stale; scaffolded ones never
  are. The scaffolded service has `XsService::new(pool)`, which
  `register_services` calls.
- Access, owners and hidden columns in API features: handlers take
  `AuthUser` (`[api.access]` `user`, the default; `401` without a valid
  token) or `AdminUser` (`admin`; `403` for a caller who isn't an admin),
  or no caller (`public`). On an owned table (`[api.owner]`) the service
  methods take the `AuthUser`, every read is scoped to the caller's id,
  `update_in` and `remove_in` lock the caller's row before writing (another
  caller's row is a `404`), and `create` sets the owner, which neither body
  holds (`CreateXDto::into_new(owner)`). `[api.hidden]` columns stay in
  both bodies but never appear in `XResponse`. `listing.rs` gains
  `get_owned` and `lock_owned`.
- API features serve the rest of ADR 0016's routes, each skippable in
  `[api.skip]`: `GET /by-<column>/{value}` per single-column unique key that
  isn't hidden; `POST /search` (`{filter, sort, order, page, per_page}`)
  with `api.search_endpoint`, whose filter is the data layer's less hidden
  columns (a `422` naming the field) and relationship filters; relationship
  routes to tables with a feature (`GET /{key}?include=<name>` for a to-one,
  `GET /{key}/<name>` for a to-many, answering a bare JSON array, and for
  a many-to-many `PUT /{key}/<name>` sync and `PUT`/`DELETE
  /{key}/<name>/{other}`), answering
  through the related feature's response under its access and owner; and
  on soft-delete tables `POST /{key}/restore` and `DELETE
  /{key}?permanent=true`. Versioned tables answer `ETag` and need `If-Match`
  on `PATCH` (`428` without, `412` when stale); tenant-scoped tables take
  `Tenant<T>`. `shared/` gains `response::etag` and
  `validation::{IfMatch, IdList}`; `generated.rs` re-exports the responses
  relationship routes use, and `listing.rs` gains the search reads,
  `owned_by`/`owned`/`retain_owned`, and the join tables' writes.
- `rok sqlc init [DIR]` scaffolds the actix-web application skeleton of
  ADR 0016 (`rok-data-sqlc.toml`, `migrations/`, `src/{lib,main,app,config,error}.rs`,
  `src/shared/{mod,auth}.rs`, `src/features/mod.rs` and `AGENTS.md`), each
  file only when absent. It generates nothing. The application is a library
  crate (`lib.rs` declares the modules `pub`, `main.rs` starts it), so a
  fresh project passes `cargo clippy --all-targets -- -D warnings`.
- `rok sqlc init` after `cargo new` replaces cargo's untouched default
  `src/main.rs` (hello world) with the scaffolded one and says so; any other
  `src/main.rs` is kept.
- `rok sqlc check-arch` (`a`) checks a project against ADR 0016's
  architecture, reading `api.output` and `api.db_path` from
  `rok-data-sqlc.toml` (`-c/--config`): the feature layout (eight files, no
  `utils`/`helpers`/`common`, no unknown top-level directory under `src/`;
  warnings under `-l/--lenient`) and, parsing the source with `syn`, rules
  1, 2, 4, 5 and 6 and the cross-feature write rule. It prints each
  violation as `path:line: rule: message` and exits 1 if there are any.
  `init`'s `AGENTS.md` states exactly the rules it enforces.
- Scaffolded files: a planned file starting with the scaffold header is
  written only when absent, never rewritten, never reported as changed and
  never stale; `--check` reports a missing one. `Report::scaffolded` lists
  the ones written.
- `primary_key_ids = "cuid2"` (`IdGeneration::Cuid2`, ADR 0016) makes new
  text primary keys with `cuid::cuid2()` (24 characters): the typed id gets
  `generate()`, and `create` fills a key the input leaves out. `UUID` keys
  are left alone. `db/mod.rs` hints `cuid = "2"`.
- The API guide in `docs/guide/`: the architecture guidelines, with the
  rules `check-arch` enforces, and which files are generated, scaffolded or
  yours. The README gains a "Generated APIs" section. Their samples come
  from `docs/examples/api`, a project started with `init`, which the test
  suite regenerates, compiles and runs `check-arch` on.
- The OpenAPI document of generated API features (ADR 0016), with utoipa 5
  and utoipa-swagger-ui 9, on by default and off with `[api] openapi =
  false` (`ApiConfig::openapi`), which leaves output with no utoipa at all.
  Every type a feature exposes derives `ToSchema` (query strings
  `IntoParams`), as do the data layer's typed ids, enums, sort keys, search
  conditions and `ErrorBody`. Every generated handler carries
  `#[utoipa::path]`: its absolute path, a tag per feature (the route base),
  the handler's name as operation id, its params and body,
  `security(("bearer_auth" = []))` when it needs a caller, and exactly the
  statuses it answers, each error with `ErrorBody`. `generated.rs` gains
  `GeneratedApiDoc` and the `BearerAuth` modifier (HTTP bearer, JWT).
  `init` scaffolds `src/openapi.rs` (`ApiDoc` for hand-written paths and
  `document()`, merging the generated document in), and its `app.rs`
  serves `/api-docs/openapi.json` and Swagger UI at `/docs/`. The
  dependency hints add `utoipa` (with the features the columns need) and
  `utoipa-swagger-ui` with `vendored`, so builds need no network. Health's
  handlers are `liveness` and `readiness`, the target document's
  operation ids. The guide gains an OpenAPI page.
- A list takes and documents `q` only when its table has `[api.search]`
  columns: the others' query string is the new shared `PageQuery`, which
  has no `q` (an unknown parameter is ignored, as before). The `bearer_auth`
  scheme has a description, and no OpenAPI description shows rustdoc link
  syntax.
- With `[api] openapi = false`, `rok sqlc generate` warns while the scaffolded
  `src/openapi.rs` still uses `GeneratedApiDoc` (`Report::warnings` in the
  library), and the OpenAPI guide lists the scaffolded lines to remove.
- The OpenAPI document reads like the target: `PaginatedResponse.total` has
  `minimum: 0`, and path parameters are described as "<Singular> id"
  ("User id") for a single key and "<Singular> <column words>" for a
  composite key's columns and a unique lookup's value; a many-to-many
  route's `{other}` is the related row's id.
- `rok sqlc init --template <NAME>` (`-t`) copies a ready-made project from
  the templates built into the binary (`templates/` in the repository; the
  only one is `actix`), each file only when absent and without its
  generated files, and prints the next steps. An unknown name lists the
  valid ones.
- The `actix` template (`templates/actix-api/`): the `users` and
  `audit-log` part of the axum starter's API, generated from one schema,
  with a contract test that checks the document it serves against the
  starter's (`tests/contract.rs`), API tests against PostgreSQL, and a
  README. CI regenerates it and fails on drift, and runs its lint gate,
  `check-arch` and tests.
- The rest of the API guide: getting started, configuring API features,
  customizing the generated API, the wire standard, testing, deploying,
  after a schema change, and troubleshooting, linked from the README.
  `docs/examples/api` gains a hand-written feature (`stats`, with its own
  OpenAPI path) and service and API tests with `#[sqlx::test]`, which the
  test suite runs when `ROK_DATA_SQLC_TEST_DATABASE_URL` is set.

## [0.3.0] - 2026-10-17

Validated bodies and HTTP extractors (ADR 0015). A run without the new
`validate` derive and `extractors` key generates byte-for-byte what 0.2.1
generates.

### Added

- `derive = ["serde", "validate"]` (`-d validate`, `Derive::Validate`) adds
  `validator::Validate` (validator 0.21) to every create input and update
  patch, with rules taken from the schema and English messages naming the
  column (`"name must be at most 80 characters"`):
  - `VARCHAR(n)`, `CHARACTER VARYING(n)`, `CHAR(n)` and `CHARACTER(n)` give
    `length(max = n)`, counted in characters;
  - a single-column `CHECK` comparing `char_length(col)`,
    `character_length(col)` or `length(col)` with an integer gives a
    `length` bound, combined with the declared length (the tighter bounds
    win, and `equal` when they meet);
  - a single-column `CHECK` comparing an integer or float column with a
    literal (`>=`, `>`, `<=`, `<`, either side), `BETWEEN`, or an `AND` of
    these gives `range`. A strict bound on an integer becomes inclusive
    (`> 0` is `min = 1`); on a float it stays strict (`exclusive_min`), and
    one that can't be represented exactly is dropped;
  - on a `NUMERIC` column the range is a `custom` rule calling a generated
    `<field>_in_range` function, with the code `range` and `range`'s
    `min`/`max` params, since validator's `range` can't compare `Decimal`;
  - a typed id over text implements `validator::ValidateLength<u64>`, and a
    typed id over an integer `validator::ValidateRange`, so a column's
    `length` or `range` rule applies through the id and through foreign keys
    using it. A typed id over `NUMERIC` gets no range rule.

  Patch fields are checked only when present. Other `CHECK`s, `NOT NULL`,
  enums and value lists are not rules. `db/mod.rs` names the `validator`
  dependency. `validate` without `serde` fails the run.
- `extractors = ["axum", "actix_web"]` (the JSON payload's `extractors`, the
  repeatable `-e`/`--extractor <axum|actix_web>` flag, and
  `Config::extractors` with the new `#[non_exhaustive]` `Extractor`, part of
  the stable library API) generates `db/http/`, which needs `validate`:
  - `db::http::axum::ValidatedJson<T>` (axum 0.8) and
    `db::http::actix_web::ValidatedJson<T>` (actix-web 4), extractors for
    any `T: DeserializeOwned + Validate`: a generated input or patch, or a
    hand-written request type. Each reads the body through its framework's
    own JSON extractor (`axum::Json<T>`, `web::Json<T>`), so axum's
    `DefaultBodyLimit` and actix-web's `JsonConfig` (limit and
    `Content-Type` predicate) apply; both limits default to 2 MiB;
  - `BodyRejection`, answered in the Rok error envelope
    `{"success": false, "error": {"message", "code", "details"?}}`: `400`
    `MALFORMED_BODY` for a missing or wrong `Content-Type`, bad JSON syntax,
    or a body that can't be read; `413` `PAYLOAD_TOO_LARGE` for a body over
    the framework's size limit, with the message
    `"the request body is over the size limit"` from both; `422`
    `VALIDATION_FAILED` with serde's message and no `details` for JSON that
    doesn't fit the type; `422` `VALIDATION_FAILED` with validator's errors
    as `details` for a broken rule. Each framework's rejection is classified
    in its module, with the message taken from serde's own error (never the
    framework's text), so both answer the same body with the same status,
    code and message;
  - `ErrorBody`, `ErrorPayload`, the three codes,
    `BodyRejection::from_serde`, and `validated(body)`, which checks a
    deserialized body against its rules.

  With actix-web, no `JsonConfig` error handler is needed. `db/mod.rs` names
  the `axum`, `serde_path_to_error` (whose error wraps serde's inside axum's
  rejection), `actix-web` and `serde_json` dependencies. `extractors` without
  `validate` fails the run, and an unknown framework lists the valid ones.
  With `extractors`, `http` is a reserved module name: a table whose module
  would be `http` fails with an error that says so (rename it with
  `[modules]`). Without `extractors`, such a table generates as before.
- The schema model keeps each text column's declared length and the simple
  `CHECK` bounds above, through `CREATE TABLE`, `ADD COLUMN`, `ALTER COLUMN …
  TYPE` (which replaces or clears them), `ADD CONSTRAINT`, and renames.
- The README has a "Validated bodies and HTTP extractors" section, from the
  new example crate `docs/examples/validated-api`.

## [0.2.1] - 2026-10-17

Additive: everything that generated with 0.2.0 generates the same way.

### Added

- `[conventions] primary_key_ids` (`Conventions::primary_key_ids`, the new
  `IdGeneration`): `"uuid_v4"`, `"uuid_v7"`, or `"ulid"` makes new primary
  keys in the generated code for tables keyed by one `UUID` column (or, with
  `"ulid"`, one text column). The typed id gets `generate()`, the create
  input takes the key as `Option<Id>`, and `None` inserts a new id, even over
  a database default. `db/mod.rs` names the `uuid` features or the `ulid`
  crate it needs. Off by default and not in the `rok` preset.
- `MONEY` (`PgMoney`), `INT4RANGE`, `INT8RANGE`, `NUMRANGE`, `DATERANGE`,
  `TSRANGE`, `TSTZRANGE` (`PgRange<T>`), `INET` and `CIDR`
  (`ipnetwork::IpNetwork`), and `MACADDR` (`mac_address::MacAddress`), in
  columns and query-file casts, with equality filters. The dependency hints
  name SQLx's `ipnetwork` and `mac_address` features. `PgMoney` and
  `PgRange` have no serde support, so like `INTERVAL` they fail a run with
  `derive = ["serde"]`.
- Partial unique indexes as `ON CONFLICT` targets in query files, when the
  target repeats the index's condition:
  `ON CONFLICT (email) WHERE deleted_at IS NULL DO …`. The soft-delete
  convention's live-row indexes can now be upsert targets. A target that
  matches only a partial index says which `WHERE` to add.
- `WITH RECURSIVE` in query files: a recursive CTE (`anchor UNION [ALL]
  term`) takes its columns from the anchor, and the recursive term must
  match them.
- `ALTER TABLE` forms that leave the model as it is no longer fail their
  table: `ALTER COLUMN … SET STATISTICS`, `SET STORAGE`, `SET COMPRESSION`,
  `SET`/`RESET (…)`, `SET GENERATED`, `RESTART` and other identity options;
  `RESET (…)`, `CLUSTER ON`, `SET WITHOUT CLUSTER`, `SET TABLESPACE`,
  `SET [UN]LOGGED`, `[NO] FORCE ROW LEVEL SECURITY`, `ALTER CONSTRAINT`,
  `SET (…)`, `REPLICA IDENTITY`, `OWNER TO`, `ENABLE`/`DISABLE
  TRIGGER`/`RULE`, and `VALIDATE CONSTRAINT`.
- `ALTER COLUMN … DROP IDENTITY` and `DROP EXPRESSION` act like `DROP
  DEFAULT`, and `TYPE … COLLATE` like `TYPE`.
- `ALTER TABLE … RENAME CONSTRAINT` renames a primary key or unique
  constraint, and `ALTER INDEX`, `ALTER SEQUENCE`, `ALTER VIEW` and `ALTER
  MATERIALIZED VIEW … RENAME TO` are followed for default constraint names.
  Renaming a key constraint's index renames the constraint.
- A migration with a `BEGIN ATOMIC` function or procedure body, a `DO`
  block, or another `ALTER` of an index, sequence or view that the SQL
  parser can't read is now read past instead of stopping the run.
- Prebuilt binaries. Pushing a `vX.Y.Z` tag runs the Release workflow, which
  builds rok-data-sqlc for Linux (x86_64 and aarch64, static musl), macOS (x86_64
  and Apple silicon), and Windows (x86_64 and ARM64) and publishes the
  archives, each with a `.sha256` file and a combined `SHA256SUMS`, as a
  GitHub release whose notes are this file's section for the version.
- `install.sh` (Linux, macOS) and `install.ps1` (Windows) install the binary
  for the current platform from a GitHub release and verify its checksum.
- Release builds strip debug symbols, which shrinks the binary.

## [0.2.0] - 2026-10-17

### Breaking changes to the generated API

- On a table with the tenant column, `{Table}Filter` has no field for the
  column (including its `*_is_null` field) and `{Table}SortKey` no variant
  for it. `search` is already scoped to the tenant parameter, so these could
  only repeat the scope or match nothing. Relationship filters are unchanged.
- An enum primary key gets a typed id wrapping the enum
  (`PriorityNoteId(pub types::TicketPriority)`), which is `Copy`, `Eq` and
  `Hash` but not `Ord` or `Display`. The key column, key parameters, the
  `search_after` cursor, and foreign keys into the table take it instead of
  the bare enum.
- With `derive = ["serde"]`, a patch deserialized from JSON tells an absent
  field (leave the column alone) from `null` (set it to `NULL`); before, both
  read as `None`. Serializing a patch skips its `None` fields, so `{}` is an
  empty patch. A nullable input field with a default reads `null` as
  `Some(None)` (insert `NULL`) the same way. Both use the new generated
  `db/nullable.rs`, so a table can no longer be named `nullable`.
- With `derive = ["serde"]`, `{Table}Filter` (including its relationship
  fields), `{Table}SortKey`, and `Direction`, `Sort`, `Ordered`, `Equality`,
  and `Text` in `db::search` derive `Serialize` and `Deserialize`. A filter
  field absent from the JSON adds no condition. The enums among them use
  snake_case variants in JSON (`{"ge": 400}`, `"desc"`, `"price_cents"`).
- A `SERIAL`, `BIGSERIAL`, or `SMALLSERIAL` column off the primary key is
  `NOT NULL`, as in PostgreSQL: its row field is `i32`/`i64`/`i16` instead
  of `Option<…>`, and inserts still leave it to its sequence. After
  `ALTER COLUMN … DROP DEFAULT` it is a required `NOT NULL` input, and a
  managed one has no source of values, which fails its table.

### Added

- The `[modules]` config section (also in the JSON payload, and
  `Config::modules` in the library) renames a table's module, by
  schema-qualified or SQL name: `"audit.users" = "audit_users"`. The row
  struct, typed id, and key struct follow the new name, so same-named tables
  of two schemas both generate, each with its own typed id. An entry naming no
  table, or a name that isn't a snake_case Rust identifier, fails the run.
- Generation fails with a diagnostic when a resource would import two typed
  ids of one name (from tables like `user` and `users`), or an imported typed
  id named like one of the resource's own items, or any file would import
  two other items of one name (the row structs and filters of `category` and
  `categories`, say). Before, the generated code didn't compile.
- `INTERVAL` maps to `sqlx::postgres::types::PgInterval`, `BYTEA` to
  `Vec<u8>` (borrowed as `&[u8]` in key, lookup, and query parameters), and
  `TIME WITH TIME ZONE`/`TIMETZ` to `sqlx::postgres::types::PgTimeTz`, in
  columns, arrays, query-file casts (`$1::interval`), and result columns.
  All three get equality filters: SQLx implements no `PartialOrd` for
  `PgInterval` or `PgTimeTz`.
- With `derive = ["serde"]`, a table or query that would put `PgInterval` or
  `PgTimeTz` (which implement no serde traits) into a serde-deriving item
  fails the run with an error naming the column or parameter.
- In query files, a cast to an enum type infers the generated enum:
  `$1::product_status` and `$1::app.ticket_state` take `types::X`, and
  `$1::product_status[]` takes `&[types::X]`. Such casts were rejected.
- `ALTER TYPE x RENAME TO y` renames the enum: its Rust name follows the new
  name (schema-prefixed only on a clash, as for `CREATE TYPE`), its
  `sqlx(type_name)` becomes `y`, and the columns using it keep working. The
  clause used to fail every table using the type.
- `ALTER TABLE … ALTER COLUMN` is applied instead of failing the table:
  `[SET DATA] TYPE` (with or without `USING`), `SET`/`DROP NOT NULL`,
  `SET`/`DROP DEFAULT`, and `ADD GENERATED … AS IDENTITY`. The column ends
  up in its final shape, and its primary key, unique keys, and typed id
  follow a type change. A change rok-data-sqlc can't apply (a type with no
  mapping, a serial type, `DROP NOT NULL` on a primary key column, identity
  on a nullable column, a missing column) still fails the table with a
  `path:line` diagnostic. Dropping a managed column's default can now leave
  it without a source of values, which fails its table.

### Changed

- A table that a `--lenient` run skips keeps the resource directory an
  earlier run generated for it, byte for byte, and `db/mod.rs` keeps
  declaring it (and `UpdateOutcome`, when its `update` uses it). Those files
  are no longer stale: a successful run does not remove them, and a lenient
  `--check` reports the table's warning but no drift. A table the schema no
  longer defines (dropped, or never created) is still stale (ADR 0001).
  The rest of the output is generated against the kept table's last good
  shape (its model before the statement that broke it), so the crate still
  compiles: `types.rs` keeps its enums, foreign keys into it keep its typed
  id, and relationships and filters still reach it. The kept directory is
  the table's module, `[modules]` renames included. A table that fails at
  its own `CREATE TABLE`, or whose value lists can't be generated, has no
  such shape, so its earlier files are stale.
- A managed column with no source of values fails its table with a
  `path:line` diagnostic: one that is `NOT NULL`, has no `DEFAULT`, is not
  serial, identity, or `GENERATED`, on a table with no row-level
  `BEFORE INSERT` trigger. Inserts leave managed columns out, so such a
  table could never be inserted into. `CREATE TRIGGER` and `DROP TRIGGER`
  are now read to find those triggers (per table; function bodies are not
  inspected). Nullable managed columns are unaffected. When a later
  migration takes the source away (`ALTER COLUMN … DROP DEFAULT`, or
  `DROP TRIGGER` of the last `BEFORE INSERT` trigger), the diagnostic
  points at that statement.
- `--resource` (and the `resource` key) is described as what it does: it
  generates the named table and regenerates every resource already
  generated in the output directory; tables not generated yet stay
  ungenerated.
- An `ALTER TABLE` the SQL parser can't read (`ALTER COLUMN … DROP
  IDENTITY`, `SET STATISTICS`, `SET STORAGE`, `DROP EXPRESSION`,
  `SET GENERATED`, `TYPE … COLLATE`) fails only the table it alters, at its
  `path:line`, so `--lenient` skips that table and reads on. Any other
  statement the parser can't read still stops the run, now at its
  `path:line`. Before, either stopped the run with a syntax error for the
  whole file.

### Fixed

- A joined read (`get_with_<relationship>`) of a table whose column names
  come within two bytes of PostgreSQL's 63-byte identifier limit works:
  such a column is selected under a shortened alias that names its
  position (`t3_<name>`), where the `t_`/`r_` prefix used to make
  PostgreSQL cut two aliases to the same name. The `is_empty` of a patch and
  the `column` of a sort key are also laid out as rustfmt lays them out for
  such long names.
- `ON CONFLICT ON CONSTRAINT` accepts the names PostgreSQL gives unnamed
  primary key and unique constraints when a default name is already taken
  by another relation in the same schema (`<table>_pkey1`): a table, another
  table's constraint, an index (named, or PostgreSQL's default
  `<table>_<columns>_idx`), a sequence (`CREATE SEQUENCE`, or a serial
  column's `<table>_<column>_seq`), or a view or materialized view, until it
  is dropped. Before, only the table's own constraints counted.
- A table whose search names would collide only through the tenant column
  generates: the tenant column has no `*_is_null` filter field and no sort
  key variant, so it collides with nothing. The check now runs after the
  conventions are applied.

## [0.1.0] - 2026-10-17

First release on crates.io. The entries below include generated-API changes
that break code written against earlier development snapshots.

### Breaking changes to the generated API

- Generated code targets SQLx 0.9 instead of 0.8: the consuming crate needs
  `sqlx = "0.9"` (same feature names) and Rust 1.94 or newer, SQLx 0.9's
  minimum. Search code takes `&mut sqlx::QueryBuilder<sqlx::Postgres>`, which
  has no lifetime parameter in 0.9.
- Struct names are singular: `Product`, `NewProduct`, and `UpdateProduct`
  for a `products` table, instead of names built from the plural table name.
- Generated CRUD functions take a generic `executor: E` (any
  `sqlx::PgExecutor`, such as a pool, a connection, or a transaction) as
  their first argument, instead of a concrete `&PgPool`.
- `get_by_id`, `exists_by_id`, `update`, and `delete` take one parameter per
  primary-key column, so tables with composite keys get the same shape as
  single-key tables.
- `read::search_after` takes a typed `Limit` (clamped to
  `1..=MAX_PAGE_SIZE`, 500) instead of a bare integer. `Page` is built on
  `Limit`, and `Page::next` saturates instead of overflowing.
- `read::search` is generated only when the table has at least one sort key,
  and `filter.rs` only when `search` or `search_after` exists.
- `list` orders rows by primary key.
- Generated SQL keeps schema qualifiers (`app.products`) and lists columns
  explicitly; partial updates and inserts with defaulted columns are built
  with `sqlx::QueryBuilder`.
- `NUMERIC`/`DECIMAL` columns map to `rust_decimal::Decimal`.
- With more than one query input (or a query directory), queries are
  generated as `db/queries/<stem>.rs`, one module per file, instead of a
  single `db/queries.rs`. A lone query file still generates `db/queries.rs`.
- Text columns restricted by a value-list `CHECK` use a generated enum
  (`types::OrderStatus`) instead of `String`.
- Text keys are borrowed: `get_by_id`, `exists_by_id`, `update`, `delete`,
  and unique-key lookups take a text key as `&` its typed id
  (`&CurrencyId`), a key struct with a text column as `&OrderItemKey`, and
  `search_after` takes `Option<&…>` for such keys. `Copy` keys stay by value.
- Query-file functions take text parameters as `&str` and array parameters
  as slices (`&[T]`), also inside `Option`. Parameter structs (more than
  three parameters) stay owned.
- Query-file functions use typed ids: a parameter typed by a primary key or
  foreign key column takes the table's typed id (`category_id: CategoryId`,
  `&CurrencyId` for a text key, `&[CategoryId]` for `= ANY($1)`), and a
  result column taken straight from such a column decodes as it. A
  parameter compared with two different ids, and computed expressions, keep
  the plain type.

### Breaking changes to the library

- `rok_data_sqlc::generate(&Config) -> Result<Report>` is the stable entry point.
  The `generator`, `output`, and `schema` modules are now private, and
  `cli`, `parser`, and `queries` are hidden from the documentation and not
  covered by semver.

### Added

- Models, reads (`get_by_id`, `exists_by_id`, `list`, `list_page`, `count`),
  and writes (`create`, `update`, `delete`) for every table in PostgreSQL
  DDL, with `@generated` overwrite protection and a schema snapshot.
- Composite primary keys, array columns, `TIME`, and `NUMERIC`.
- PostgreSQL enum types (`CREATE TYPE ... AS ENUM`, `ALTER TYPE ... ADD/RENAME
  VALUE`) become Rust enums in a generated `db::types` module, used by
  rows, inputs, filters, sort keys, and query files. `types` is a reserved
  module name.
- Value-list columns (text columns restricted by `CHECK (col IN ('a', ...))`,
  inline, as a table constraint, or added by `ALTER TABLE`) become Rust enums
  in `db::types`, named after table and column (`OrderStatus`), that encode
  as text and keep each exact value.
- Typed search: per-table `Filter` and `SortKey` types, `search`, and keyset
  pagination with `search_after`.
- Unique-key lookups: every unique constraint (inline, table-level, or added
  by `ALTER TABLE`) and unique index on plain columns gets
  `read::get_by_<col>[_and_<col>...]`, returning `Option<Row>`. Text key
  columns are borrowed (`&str`, or `&` a text typed id), other columns by
  value.
  Expression indexes and partial indexes get no lookup.
- Annotated query files (`-- name: X :one|:maybe_one|:many|:exec|:execrows`)
  generate typed query functions, including `WITH` (CTE) queries.
- A query file's `ON CONFLICT (cols)` target must be a unique key of the
  table (the primary key, a unique constraint, or a full unique index, in
  any column order); otherwise generation fails with a `path:line`
  diagnostic listing the table's unique keys. `ON CONFLICT ON CONSTRAINT
  name` must name the table's primary key or a unique constraint, by its
  declared name or PostgreSQL's default (`<table>_pkey`,
  `<table>_<columns>_key`); otherwise the diagnostic lists the valid names.
- `--schema` accepts files or migration directories and can be repeated;
  `ALTER TABLE` statements in later migrations change earlier tables.
  Diagnostics print `path:line:`. `ALTER TABLE … ADD CONSTRAINT … CHECK`
  that is not a value list is ignored, like such a `CHECK` in `CREATE
  TABLE`, instead of failing the table.
- `DROP TABLE` (also `IF EXISTS`, schema-qualified, or several names) in a
  later migration removes the table, so its generated files are stale and
  get removed. Foreign keys other tables hold to it are dropped as with
  `CASCADE`: the column keeps its raw type and makes no relationship.
  `DROP TYPE` removes an enum. With `CASCADE`, its columns are dropped too.
  Without it, tables still using the type fail.
- `--queries` accepts files or directories and can be repeated.
- `rok-data-sqlc.toml` discovery (or `-c/--config`) and a `-j/--json`
  configuration payload, layered as flags > payload > file > defaults.
- A `[conventions]` section in `rok-data-sqlc.toml` and the JSON payload
  (`rok_data_sqlc::Conventions` in the library), off by default, with a `rok`
  preset for the Rok ERP. Managed columns (`managed_columns`, such as
  `updated_at` and `row_version`) are left out of create inputs and update
  patches and kept in rows. See ADR 0010.
- Tenant scoping (`tenant_column`, `organization_id` in the `rok` preset). On
  a table with that column, every generated function takes the tenant id
  right after the executor and adds `<column> = <tenant>` to every statement:
  reads, unique-key lookups, `search`, `search_after`, `create`, `update`
  (versioned or not, empty patch included), and every delete. The column is
  left out of create inputs (it is set from the parameter) and update patches
  (it never changes). When it is part of the primary key, the tenant
  parameter replaces that key column, and unique-key lookups leave it out of
  their name (`UNIQUE (organization_id, code)` gives `get_by_code`).
- `db::session` (generated when `tenant_setting` or `user_setting` is set):
  `set_tenant(&mut tx, id)` and `set_user(&mut tx, id)` run a
  transaction-local `set_config`, for row-level security policies and audit
  triggers. `session` is a reserved module name.
- A version column (`version_column`, `row_version` in the `rok` preset) for
  optimistic concurrency. On a table that has it, `update` takes
  `expected_version` and returns `UpdateOutcome<Row>` (`Updated(row)`,
  `NotFound`, or `VersionConflict`), defined once in `db/mod.rs`. The check
  is part of the `UPDATE`'s condition, and one statement tells a stale
  version from a missing row. The version column is always managed. A
  soft-deleted row is `NotFound`. Tables
  without the column keep the plain `update`. See ADRs 0006 and 0010.
- Soft delete (`soft_delete_column`, `deleted_at` in the `rok` preset). On a
  table with that column, which must be a nullable `TIMESTAMP` or
  `TIMESTAMPTZ` (otherwise generation fails), `delete` sets it to `now()` on a
  live row and reports whether a row changed. `restore` clears it, and
  `delete_permanently` removes the row. Every generated read skips
  soft-deleted rows. The search filter gains `include_deleted: bool`, and
  `update` on a deleted row returns `None`. The column is left out of create
  inputs and update patches. A partial unique index whose condition is
  exactly `<column> IS NULL` now gets a `get_by_<cols>` lookup. Query files
  are not rewritten.
- Relationships inferred from foreign keys between the tables generated
  together (ADR 0014): a belongs-to per foreign key (`category_id` →
  `category`), its inverse has-many named after the child module
  (`products`, or `invoices_by_created_by_user` when the child has several
  foreign keys to the parent), a has-one when the foreign key column is a
  unique key, and `parent`/`children` for a `parent_id` self-reference. The
  tenant column's foreign key is not a relationship. Each resource with
  relationships gets `relationships.rs` with a `load_<name>` loader per
  relationship: it takes already-fetched rows and loads the relationship for
  all of them in one `= ANY($n)` query, returning a `HashMap` keyed by the
  joining key (typed id where there is one): one row per key for to-one,
  a list per key (empty when nothing relates) for to-many. Loaders on a
  tenant table take the tenant and skip soft-deleted rows. Two relationships
  of a table with the same name fail generation, naming both.
- Many-to-many relationships through join tables: a table whose two
  non-tenant `NOT NULL` foreign keys, to two other different tables, form a
  unique key (payload columns allowed) relates those tables many-to-many,
  named after the far table (`permissions`, or
  `permissions_through_role_permissions` when the name is taken or several
  join tables lead there). Its `load_<name>` loader goes through the join
  table in one query and respects both tables' tenant and soft-delete
  columns. The join table's `write.rs` gains `attach` (idempotent, returns
  whether the pair is new), `detach`, and `sync(near, &[far])`, which
  replaces the set in one statement, so it is atomic in the caller's
  transaction. On a soft-delete join table `detach` soft-deletes and
  `attach`/`sync` restore the soft-deleted row instead of inserting another.
  `attach` and `sync` are left out when another column needs a value. A
  table with two foreign keys but no unique pair (like `user_roles`) is not a
  join table. Opting out one side of a many-to-many removes the other;
  opting out a join table's foreign key removes the pair through it and
  its join writes. A many-to-many relationship filter goes through a live
  join row in the same tenant.
- Joined reads for to-one relationships (ADR 0014): a table with a primary
  key gets `get_with_<name>` in `relationships.rs` for each belongs-to and
  has-one, reading the row and its related row in one query into a
  `{Row}With{Relationship}` struct (`ProductWithCategory`). The related row
  is an `Option` unless a NOT NULL foreign key guarantees it (a has-one, a
  nullable foreign key, or a related table with a tenant or soft-delete
  column makes it optional). Columns are selected under per-side aliases, so
  shared column names decode unambiguously; the related table's tenant and
  soft-delete conditions sit in the join's `ON` clause, so a deleted or
  other-tenant parent reads as `None` instead of hiding the child.
- Relationship filters (ADR 0014): a resource's `{Struct}Filter` gains an
  `Option<Box<{Related}Filter>>` field per relationship
  (`ProductFilter { category: Some(Box::new(CategoryFilter { name, .. })), .. }`),
  rendered as a correlated `EXISTS` subquery so rows never repeat; for a
  to-many relationship it means "at least one related row matches". An
  empty related filter requires a related row (a `NULL` foreign key has
  none). Soft-deleted related rows never match, and a related tenant
  table's rows must share the searched row's tenant; an untenanted
  resource gets no filter field into a tenant table. Filters nest up to
  `db::search::MAX_RELATIONSHIP_DEPTH` (4) deep, and `search` fails with
  `sqlx::Error::InvalidArgument` beyond it. A relationship named like one of
  the filter's column fields fails generation, naming both; renaming it
  in `[relationships]` resolves the clash, and filters use renamed names
  and skip opted-out relationships.
- A `[relationships]` section (config file, JSON payload, or
  `Config::relationships` with `rok_data_sqlc::RelationshipChange`) renames or
  opts out inferred relationships, by table and inferred name:
  `[relationships.invoices]` with `created_by_user = "author"` generates
  `load_author`, and `approved_by_user = false` generates nothing for that
  foreign key on either side. Entries are checked against the whole schema,
  so `--resource` runs accept the same configuration; an entry naming an
  unknown table or relationship, an invalid name, or a rename that collides
  fails, naming the entry (`relationships.invoices.created_by_user`).
- `-k/--check` reports drift without writing and exits 1 when generated
  files are missing, changed, or stale. Successful runs remove stale
  generated files.
- Generation and `--check` ignore line endings: migrations and query files
  are read with CRLF normalized to LF (so the schema snapshot is the same on
  every platform), and generated files that differ only in CRLF/LF count as
  unchanged.
- Every generated public item is documented, and generated code passes
  `missing_docs` and `cargo clippy -- -D warnings` with a selected set of
  pedantic lints; both fixture crates enforce the bar (ADR 0013).
- `derive = ["serde"]` (config file, JSON payload, `-d/--derive serde`, or
  `Config::derive` with `rok_data_sqlc::Derive::Serde`) adds
  `serde::Serialize`/`Deserialize` to rows, typed ids (transparent), key
  structs, enums (by database label), inputs, patches, joined-read structs
  (`ProductWithCategory`), and query rows and parameters, and adds `serde` to the dependency hints. Without it,
  generated code does not mention `serde`.
- Short aliases for every command and flag; `--help` and `--version` exit 0.
- Crate metadata, license files, this changelog, and CI on Linux (Rust 1.88,
  1.94, and stable, with PostgreSQL), macOS, and Windows.

### Changed

- The minimum supported Rust version is 1.88 (`rust-version = "1.88"`).
  Generated code needs Rust 1.94 (SQLx 0.9), so on toolchains below 1.94
  the test suites that compile generated code print a skip notice instead
  of running.
- Usage errors print clap's message once and exit 1.
- Diagnostics and file lists print paths with `/` on every platform, and
  paths under the current directory relative to it (also when they come
  from `rok-data-sqlc.toml`).

[Unreleased]: https://github.com/ateeq1999/rok-data-sqlc/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/ateeq1999/rok-data-sqlc/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/ateeq1999/rok-data-sqlc/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/ateeq1999/rok-data-sqlc/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/ateeq1999/rok-data-sqlc/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ateeq1999/rok-data-sqlc/releases/tag/v0.1.0
