# rok-data

PostgreSQL data tooling built around one pipeline: a developer writes SQL,
`rok` generates a fluent, chainable data layer, migrations, and a running
API from it.

## Language

### Pipeline

**Unified data layer**:
The single fluent, chainable ORM style that every generator emits.
_Avoid_: ORM layer, sqlx layer, "the codegen style"

**Schema dir**:
The folder of `.sql` files a developer writes and edits; the project's
only writable source of truth. Models, migrations, and API code derive
from it.
_Avoid_: schema files (when the folder is meant)

**SQL-first**:
The direction of truth: schema dir in, everything else derived; no
generated artifact writes DDL except the diff engine.
_Avoid_: bidirectional sync, two-way schema sync

**Drift check**:
Verification that generated files on disk still match what the current
schema dir would generate; a failing check exits non-zero for CI.
_Avoid_: stale check, codegen lint

**Legacy alias**:
An old command name kept working after the command surface is rebuilt,
forwarding verbatim into the same implementation.
_Avoid_: deprecated command, shim

**With-loading**:
Fetching related rows as part of the query chain itself (`.with(relation)`
on a Select), so the returned rows already carry their relations. Each
include runs one batched follow-up query on the same connection (or
transaction), never a query per row.
_Avoid_: eager loading (generic), lazy loading, N+1 fix

**Include**:
One relation requested for loading in a `with()` chain — either a bare
relation constant (`User::POSTS`) or an `Include` with its own shaping
(filters, ordering, limit, offset that act on the batch, not the rows).
_Avoid_: eager clause, nested with

**Loaded**:
The view a `with()` query returns: the fetched rows plus every include's
state. Typed reads go through the same relation constant that asked
(`view.loaded(User::POSTS)`, `view.try_get(..)`); a relation never
requested reads as `None`, never a panic.
_Avoid_: result map, joined rows

### Data layer

**Model**:
The row type and its verbs: one struct per table, a `#[derive(Model)]` that
gives typed column constants, relation constants and the entry points, a
`Model` trait for the read chain (`find`, `query`, `filter`, `exists`,
`chunk`…) and `ModelWrite` for the write verbs (`insert`, `update`,
`upsert`…). Calling the data layer is calling the model
(`User::find(&db, id)`, `user.posts()`, `User::popular()`); the generated
modules' plain functions remain the low-level helpers.
_Avoid_: entity, resource (for the row type), repository

**ModelCollection**:
A set of models in memory: what `get()` returns (`all()` returns the
plain `Vec`). Through `Deref` it is a `Vec`; its own verbs work by
primary key — `find`, `contains_key`, `only`/`except`, `unique`,
`intersect`/`diff` — and go back to the database: `to_query` (the rows'
keys as a `WHERE`), `fresh` (re-read them), `load`/`load_missing` (the
relations beside them, into a **Loaded** view).
_Avoid_: result set, rows (for the collection type), Laravel's untyped
Collection

**Page**:
One page of rows with what a client needs to page on: `Page<T>` holds the
`items`, the `total` across every page, the `page` (from 1) and the
`per_page` — what `Select::paginate` returns. On the wire the generated
`PaginatedResponse<T>` carries the same four fields
(`PaginatedResponse::from(page)`).
_Avoid_: window (that is the SQL side), result set

**CursorPage** / **Cursor**:
One page of a keyset-paginated query (`Select::cursor_paginate`): the
`items` and the **Cursor** — an opaque token holding the ordering values of
the page's edge — to continue from. Stable under concurrent inserts, no
`COUNT(*)`.
_Avoid_: offset (a cursor is not a row number)

**Window**:
The SQL side of a page in the generated code (`db::search::Window`): a
clamped `LIMIT` and an `OFFSET`, what a generated list query takes. A
request's page (`Pagination`) converts into one.
_Avoid_: page (for the limit/offset pair; 0.2 called it `Page`)

**Scope**:
A named, reusable constraint on a query chain: a function that takes the
chain by value and returns it (`fn popular(q: Select<User>) ->
Select<User>`). The model declares its scopes by path
(`#[rok(scopes(popular = path::to::popular))]`), and the data layer gives
the model an entry (`User::popular()`) and the chain a method
(`User::popular().active()`), so scopes compose. The model's
`default_scope` is the scope every query of that model carries.
_Avoid_: query filter (when it is named and reusable), global scope (the
model's `default_scope`)

### Pipeline pieces (deferred from the engine context)

The terms **Table**, **Resource**, **API feature**, **Template**,
**Scaffolded file**, **Drift**, **Convention**, **Preset**, and the
relationship vocabulary are owned by the engine context's glossary at
`crates/rok-data-sqlc/GLOSSARY.md`; this context reuses them with the
same meaning rather than redefining them.
