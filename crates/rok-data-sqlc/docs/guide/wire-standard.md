# The wire standard

Every generated API answers the same way: the same success shapes, the same
error body, the same list shape and the same headers. This page lists all
of them. The exchanges are real answers of
[`docs/examples/api`](../examples/api) (with the `date` and
`content-length` headers left out), and the Rust samples come from its
source.

## Success

A success answers the bare object, never an envelope:

| Route | Status | Body | Headers |
|---|---|---|---|
| `GET /` (list) | `200` | a page: `{items, page, per_page, total}` | |
| `POST /search` | `200` | a page, like the list | |
| `POST /` (create) | `201` | the row | `Location`, the row's URL; `ETag` on a versioned table |
| `GET /{id}` | `200` | the row | `ETag` on a versioned table |
| `PATCH /{id}` | `200` | the row, changed | `ETag` on a versioned table |
| `DELETE /{id}` | `204` | none | |
| `POST /{id}/restore` | `200` | the row, restored | `ETag` on a versioned table |
| `GET /by-<column>/{value}` | `200` | the row | `ETag` on a versioned table |
| `GET /{id}?include=<name>` | `200` | the row's fields and, under `<name>`, the related row or `null` | `ETag` on a versioned table |
| `GET /{id}/<name>` | `200` | the related rows, a bare JSON array | |
| `PUT /{id}/<name>/{other}`, `DELETE /{id}/<name>/{other}`, `PUT /{id}/<name>` | `204` | none | |

A created row:

```http
HTTP/1.1 201 Created
content-type: application/json
location: /api/v1/projects/01a1538f-54cf-729e-b214-0e47a960825b

{"id":"01a1538f-54cf-729e-b214-0e47a960825b","owner_id":"0190a4b2-4c1e-7d3a-9f00-000000000001","name":"Launch","open_tasks":0,"created_at":"2026-10-19T09:47:44.718664Z"}
```

A row's JSON has one field per column, named as the column, less the
`[api.hidden]` columns. Timestamps are RFC 3339 in UTC, UUIDs strings, and
a `NULL` is `null`.

## Lists

`GET /` takes `?page=&per_page=&q=&sort=&order=`:

| Parameter | Default | Meaning |
|---|---|---|
| `page` | `1` | the page, from 1; `0` is 1 |
| `per_page` | `20` | the rows a page holds, clamped to 1 to 100 |
| `q` | none | text the `[api.search]` columns are matched against (`ILIKE`, any column); only on a table that has them |
| `sort` | `created_at`, or the first sort key | one of the table's sort keys (`[api.sort]`) |
| `order` | `desc` | `asc` or `desc` |

It answers one page, with the total over every page:

```http
GET /api/v1/projects?page=2&per_page=1

HTTP/1.1 200 OK
content-type: application/json

{"items":[{"id":"01a1538f-54cf-729e-b214-0e47a960825b","owner_id":"0190a4b2-4c1e-7d3a-9f00-000000000001","name":"Launch","open_tasks":0,"created_at":"2026-10-19T09:47:44.718664Z"}],"page":2,"per_page":1,"total":2}
```

<!-- source: docs/examples/api/src/shared/response.rs -->
```rust
/// One page of a list: `{"items", "page", "per_page", "total"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PaginatedResponse<T> {
    /// The page's rows.
    pub items: Vec<T>,
    /// The page, from 1.
    pub page: u32,
    /// The most rows a page holds.
    pub per_page: u32,
    /// The rows on every page together.
    #[schema(minimum = 0)]
    pub total: i64,
}
```

Pages are offsets, so a row added while a client pages through may shift
the next page by one. Ties in the sort are broken by the primary key, so
the order is stable. `POST /search` (with `[api] search_endpoint`) takes
the same `sort`, `order`, `page` and `per_page` in its body, beside a
`filter` ([Configuring](configuring.md#search_endpoint)), and answers the
same page.

## Errors

Every error answers the same body: a snake_case `code` to branch on, a
`message` for people, and, for broken validation rules, `details` by
field.

```jsonc
{"error": {"code": "not_found", "message": "no `projects` row has this key"}}
```

| Code | Status | When |
|---|---|---|
| `bad_request` | 400 | the body isn't JSON or isn't sent as `Content-Type: application/json`; the path or the query string doesn't parse (a malformed UUID, `?page=abc`, an unknown `sort`) |
| `unauthorized` | 401 | no bearer token, or one that is invalid or expired |
| `forbidden` | 403 | an `admin` route called with a token whose role isn't `admin` |
| `not_found` | 404 | no row has the key, or it is another caller's (`[api.owner]`) |
| `conflict` | 409 | a unique or foreign-key violation, naming the constraint |
| `version_conflict` | 412 | `If-Match` holds a stale version, or none |
| `payload_too_large` | 413 | the body is over actix-web's JSON limit (2 MiB unless you set a `JsonConfig`) |
| `validation_failed` | 422 | JSON that doesn't fit the body (a wrong type, a missing field), a body breaking its rules, or a check constraint violation |
| `precondition_required` | 428 | `PATCH` on a versioned table without `If-Match` |
| `internal_error` | 500 | anything else, such as a lost database connection |

A generated route's OpenAPI document lists exactly the statuses it can
answer, each with this body (`ErrorBody`), except `500`, which any route
can answer.

The scaffolded `src/error.rs` maps each `AppError` variant to its code and
status. It is yours: add a variant there for a new error case.

The templates (`rok sqlc init --template actix` or `axum`) add four for their
account flows, and one for uploads (`POST /api/v1/media`). They also
answer `413` for an upload over its own limit, and `403` for an API key
that may not call the route:

| Code | Status | When |
|---|---|---|
| `invalid_token` | 400 | a mailed link's token is unknown, expired or already used |
| `wrong_password` | 400 | the current password a flow asks for again doesn't match |
| `breached_password` | 400 | a new password appears in a known data breach (`BREACH_CHECK=true`) |
| `too_many_requests` | 429 | over the `/auth` rate limit, `AUTH_RATE_LIMIT` requests a minute per IP; `Retry-After` gives the seconds to wait |
| `unsupported_media_type` | 415 | an upload whose type, detected from its bytes, is unknown or not in `MEDIA_ALLOWED_TYPES` |
| `payload_too_large` | 413 | an upload over `MEDIA_MAX_UPLOAD_BYTES` (10 MiB), or a profile photo over 2 MiB |
| `forbidden` | 403 | an API key on `/api/v1/auth/*` or `/api/v1/api-keys`, or a `read` key on a method other than `GET`, `HEAD` or `OPTIONS` |

These `400`s are flow failures. A body of the wrong shape (an email that
isn't one, a password outside 8 to 128 characters) is still a `422
validation_failed`, and a sign-in with bad credentials a `401`.

<!-- source: docs/examples/api/src/error.rs -->
```rust
    pub fn code(&self) -> &'static str {
        match self {
            AppError::BadRequest(_) => "bad_request",
            AppError::Unauthorized(_) => "unauthorized",
            AppError::Forbidden(_) => "forbidden",
            AppError::NotFound(_) => "not_found",
            AppError::Conflict(_) => "conflict",
            AppError::PayloadTooLarge(_) => "payload_too_large",
            AppError::Validation { .. } => "validation_failed",
            AppError::PreconditionRequired(_) => "precondition_required",
            AppError::VersionConflict(_) => "version_conflict",
            AppError::Internal(_) => "internal_error",
        }
    }
```

### 400: malformed requests

```http
HTTP/1.1 400 Bad Request
content-type: application/json

{"error":{"code":"bad_request","message":"Path deserialize error: UUID parsing failed: invalid character: found `n` at 0"}}
```

```http
HTTP/1.1 400 Bad Request
content-type: application/json

{"error":{"code":"bad_request","message":"Query deserialize error: unknown variant `owner_id`, expected `id` or `created_at`"}}
```

```http
HTTP/1.1 400 Bad Request
content-type: application/json

{"error":{"code":"bad_request","message":"the request body is not valid JSON: EOF while parsing a value at line 1 column 8"}}
```

### 401 and 403: the caller

```http
HTTP/1.1 401 Unauthorized
content-type: application/json

{"error":{"code":"unauthorized","message":"the token is invalid or expired"}}
```

Without a token the message is `a bearer token is required`, and an admin
route called by a user answers `403` with `this needs the admin role`.
These messages come from your `src/shared/auth.rs`.

An API key (the templates) that is unknown, revoked or expired, or whose
owner is disabled or deleted, answers `401` with `the API key is not
valid`. A valid key answers `403` on `/api/v1/auth/*` and
`/api/v1/api-keys`, which need a session:

```http
HTTP/1.1 403 Forbidden
content-type: application/json

{"error":{"code":"forbidden","message":"an API key can't reach this; sign in instead"}}
```

and a `read` key answers `403` with `a read API key may only GET, HEAD or
OPTIONS` on any other method.

### 404: no such row

Another caller's row on an owned table answers exactly like a missing one,
so a caller can't learn which keys exist:

```http
HTTP/1.1 404 Not Found
content-type: application/json

{"error":{"code":"not_found","message":"no `projects` row has this key"}}
```

### 409: constraints

```http
HTTP/1.1 409 Conflict
content-type: application/json

{"error":{"code":"conflict","message":"the change breaks a reference (constraint `tasks_project_id_fkey`)"}}
```

That is a project deleted while tasks still reference it. A unique
violation reads ``the value is already taken (constraint `<name>`)``. Name
your constraints in the migrations if clients branch on them.

### 422: validation

A body breaking its rules answers `details`, validator's errors by field.
The rules come from the schema (`VARCHAR(80)` gives a length rule, a
`CHECK (open_tasks >= 0)` a range rule):

```http
HTTP/1.1 422 Unprocessable Entity
content-type: application/json

{"error":{"code":"validation_failed","message":"the request body failed validation","details":{"open_tasks":[{"code":"range","message":"open_tasks must be at least 0","params":{"min":0}}]}}}
```

A `PATCH` body naming an immutable column (`[api.immutable]`), even as
`null`, breaks the rule `immutable` under that field. The field is never
ignored, though fields the body doesn't know are:

```http
HTTP/1.1 422 Unprocessable Entity
content-type: application/json

{"error":{"code":"validation_failed","message":"the request body failed validation","details":{"email":[{"code":"immutable","message":"`email` can't be changed","params":{}}]}}}
```

JSON that doesn't fit the body answers serde's message, without details:

```http
HTTP/1.1 422 Unprocessable Entity
content-type: application/json

{"error":{"code":"validation_failed","message":"invalid type: integer `5`, expected a string at line 1 column 9"}}
```

A check constraint the rules don't cover is caught by the database and
answers ``the value breaks the check `<name>` ``, without details.

### 413: too large

```http
HTTP/1.1 413 Payload Too Large
content-type: application/json

{"error":{"code":"payload_too_large","message":"the request body is over the size limit"}}
```

The templates' uploads name their own limit, in the wire standard's body
rather than the framework's plain text:

```http
HTTP/1.1 413 Payload Too Large
content-type: application/json

{"error":{"code":"payload_too_large","message":"the file is larger than 10485760 bytes"}}
```

A profile photo over 2 MiB says `the image is larger than 2097152 bytes`.

### 415: unsupported media type (the templates)

An upload's type is detected from its bytes, never from `Content-Type` or
its name. One that isn't recognised, or isn't in `MEDIA_ALLOWED_TYPES`,
answers:

```http
HTTP/1.1 415 Unsupported Media Type
content-type: application/json

{"error":{"code":"unsupported_media_type","message":"files of type application/vnd.microsoft.portable-executable are not accepted"}}
```

Bytes of no type it knows say `the file type could not be recognized`.

### 500: everything else

The cause is logged with `tracing` and never sent; the message is always
`something went wrong`. A tenant-scoped route reached without a tenant (a
missing middleware) is one, since it is a wiring bug, not the caller's
fault.

## Versioned rows

On a table with a version column (`[conventions] version_column`), every
answer holding one row carries the version as an `ETag`, in quotes:

```http
HTTP/1.1 200 OK
content-type: application/json
etag: "3"
```

`PATCH` needs it back in `If-Match`, exactly as received:

| `If-Match` | Answer |
|---|---|
| missing | `428 precondition_required`, `` `If-Match` must hold the row's `ETag` `` |
| not a quoted version (`*`, `W/"3"`, `3`) | `412 version_conflict`, `` `If-Match` holds no version of the row `` |
| an older version | `412 version_conflict`, `the row has changed since this version` |
| the current version | `200`, the changed row with its new `ETag` |

On a `412`, read the row again and retry with its new `ETag`.

## Health

`/health/live` and `/health/ready` sit outside the API prefix and need no
token:

| Route | Answer |
|---|---|
| `GET /health/live` | `200 {"status":"ok"}` while the process answers |
| `GET /health/ready` | `200 {"status":"ready"}` when the database answers `SELECT 1`, else `503` with code `service_unavailable` |

[Deploying](deploying.md#health-checks) says which to point each probe at.

## Routes

Every feature serves its routes under `[api] prefix` (`/api/v1`) and its
route base, a plural kebab-case name (`/projects`, `/audit-log`). Keys are
path segments (`/{id}`; a composite key is one segment per column), an
update is `PATCH` with only the fields to change, and `null` sets a
nullable column to `NULL`. The README's [route
table](../../README.md#generated-apis) lists every route and the `[api.skip]`
name that drops it.
