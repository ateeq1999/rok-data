# Migrating to `rok-data`

Per-repository note: what your project needs to change to move from one
of the four source repositories to the consolidated workspace, based on
what was actually renamed, removed, or unified. Everything not listed is
unchanged — same APIs, same test suites, same behavior (verified by
porting the original tests).

The one section that is not about a repository lineage is **To 0.3.0**,
the upgrade from 0.2.0 to 0.3.0: the runtime's public surface and the
generated output both change there.

## To 0.3.0

0.3.0 is a breaking release: the runtime's public surface changes, and the
generated output changes with it. Regenerate once (`rok gen`, or
`rok sqlc generate` for the engine's own projects) and fix the compiler
errors the compiler finds for you. Nothing else changes — the query
builders, the executors, the hooks' behaviour and the CLI's commands,
flags, outputs and exit codes are the same.

### Renamed calls

| Before (0.2.0)        | After (0.3.0)             | What it is |
| --------------------- | ------------------------- | ---------- |
| `Model::create()`     | `Model::insert_builder()` | the insert builder: one column at a time, raw expressions, conflict handling |
| `row.save(&db)`       | `row.update(&db)`         | the row-level update by primary key; returns the stored row |
| `row.save_only(&db, Some(vec![User::NAME]))` | `row.update_only(&db, [User::NAME])` | the row-level update of the listed columns only; any list or iterator of columns, and an empty one is an error |
| `users::write::create(exec, input)` | `User::create(exec, input)` | the generated insert, now on the model |
| `users::write::update(exec, id, input)` | `User::patch(exec, id, input)` | the generated update: returns the patched row (`Result<User>`) |
| `users::write::delete(exec, id)` | `User::destroy(exec, id)` | the generated delete: `Result<()>` |
| `users::write::restore` / `delete_permanently` | `User::restore` / `User::delete_permanently` | the soft-delete pair, now on the model: `Result<()>` |
| `user_roles::write::attach` / `detach` / `sync` | `UserRole::attach` / `detach` / `sync` | a join table's pair writes, on the join model |

The module functions (`users::write::create`, …) still compile for one
release: each forwards to the model verb and is `#[deprecated]`, so the
compiler points every call site at its replacement.

**One "not found" convention.** A verb that targets one row by key fails
with `rok_data::Error::NotFound` when no row has it — the generated
`patch`, `destroy`, `restore` and `delete_permanently` exactly like the
model's own `update`, `delete` and `find_or_fail`. Optional lookups stay
optional (`find` and the generated `get_by_*` return `Option`). A versioned
`patch` keeps its `UpdateOutcome`, whose `VersionConflict` is not an error.

<!-- not compiled: 0.2 code, shown for comparison -->
```rust,ignore
// before (0.2)
let user = match users::write::update(&db, id, patch).await? {
    Some(user) => user,
    None => return not_found(),
};
if users::write::delete(&db, id).await? == 0 {
    return not_found();
}
```

<!-- source: crates/rok-data-sqlc/tests/fixture-app/src/one_import.rs -->
```rust
// after
let user = match User::patch(&db, id, patch).await {
    Ok(user) => user,
    Err(error) if error.is_not_found() => return not_found(),
    Err(error) => return Err(error.into()),
};
User::destroy(&db, id).await?; // `?` carries NotFound to your error type
```

<!-- not compiled: 0.2 code, shown for comparison -->
```rust,ignore
// before (0.2)
let user = User::create()
    .set(User::EMAIL, "a@b.c")
    .exec(&db)
    .await?;
user.save(&db).await?;
```

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
let user = User::insert_builder()
    .set(User::EMAIL, "a@b.c")
    .exec(&db)
    .await?;
user.update(&db).await?;
```

`save` is **removed, not aliased**: it is the one verb whose
meaning is ambiguous (insert *or* update), and a non-optional typed key
leaves the runtime nothing to decide on. Use the explicit verbs — `insert`,
`update`, `upsert`, `upsert_on` — instead.

The generated data layer changes with the runtime: its write verbs move
onto the model and take the model's verb names (the table above):
`update` becomes `User::patch` (the instance verb `update` already means
"write this row"), `delete` becomes `User::destroy`, and a missing row is
`Error::NotFound` rather than a `None` or a `0` (*One "not found"
convention*, above): an `assert!(write::delete(...).await.unwrap())`
becomes `User::destroy(...).await.unwrap()`. The instance `update`,
`insert_builder()`, the query builders, `restore` and `delete_permanently`
keep their names. Regenerate and let the compiler find the call sites.

The **API layer keeps its 0.2 names**: the generated handlers and service
methods (`update`, `remove`, `remove_permanently`, `Remove{Row}Query`),
the OpenAPI `operationId`s and the `[api.skip]` names are unchanged, so a
`service.rs` override and an OpenAPI client keyed on `operationId` keep
working.

### New verbs

| Verb                | Signature                                  | Notes |
| ------------------- | ------------------------------------------ | ----- |
| `Model::exists`     | `fn exists(executor) -> Result<bool>`      | does the table have a row? the model's own scope applies (soft deletes, default scope, tenant) |
| `Model::where_`    | `fn where_(expr) -> Select<Self>`         | another name for `filter` |
| `Select::where_`   | `fn where_(expr) -> Self`                 | another name for `filter`, on the chain |
| `Select::get`       | `fn get(executor) -> Result<ModelCollection<Self>>` | the rows as a `ModelCollection`; `all` keeps the `Vec` |
| `Select::chunk`     | `fn chunk(session, size, f) -> Result<()>` | the rows in windows of `size`, one query each, so a large table need not fit in memory |

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
// `where` is a Rust keyword, so `where_` is the spelling; `filter`
// stays the canonical verb and the generated code keeps using it.
let admins = User::where_(User::ROLE.eq("admin")).all(&db).await?;

// one row or many, without fetching them
if User::exists(&db).await? {
    // …
}

// the same rows as `all`, as a `ModelCollection`
let users = User::query().get(&db).await?;

// the rows in windows of 100 — `db` as a reference the `async move`
// blocks copy
let db = &db;
User::query()
    .chunk(db, 100, |rows| async move {
        for user in rows {
            archive(db, user).await?;
        }
        Ok(())
    })
    .await?;
```

`chunk` reads one window per query (`LIMIT`/`OFFSET`, ordered by the
primary key when the query has no order), so only one window is in memory
and no connection is held while the callback runs — a callback that writes
back works even with a pool of one. The callback returns `Ok(())` to go on
or `Ok(false)` to stop; the first error stops the
walk and is returned.

The other table walks, for large tables:

| Call | What it does |
| ---- | ------------ |
| `.chunk_by_id(db, 100, f)` | keyset windows by primary key: correct when `f` deletes or updates the rows it was handed |
| `.lazy(db, n)?` / `.lazy_by_id(db, n)?` / `.lazy_by_id_desc(db, n)?` | a `Stream` of rows, read `n` at a time |
| `.each(db, f)` / `.each_by_id(db, f)` | one row per call, a thousand per query |

### Relations

**New in 0.3.0.** The schema's foreign keys become relation attributes on
the model: the holding field carries `#[rok(belongs_to = Target)]`, and the
referenced table gets `#[rok(has_many(children = Child::FK))]` — or
`has_one` when the foreign-key column alone is a unique key there. The
derive expands them into typed relation constants and accessors, so a call
site reads them off the row:

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
// a post belongs to its author; a user has many posts
post.author().all(&db).await?;
user.posts().where_(Post::DRAFT.eq(false)).all(&db).await?;
```

The relation name follows the column: `author_id` names the relation
`author`, and a foreign key the schema does not name unambiguously states
it — `#[rok(belongs_to(author = User))]`. Soft-deleted related rows stay
out of an accessor's query, like everywhere else a read happens.

### Named scopes

**New in 0.3.0.** A model can declare **named scopes**: reusable
constraints on a query chain, named by path. The derive gives the model an
entry (`User::popular()`) and the chain a method
(`User::popular().active()`), so scopes compose:

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
#[derive(Model)]
#[rok(scopes(popular = popular, active = active))]
struct User {
    id: i64,
    views: i32,
}

// a scope is a function from a query chain to a query chain
fn popular(query: Select<User>) -> Select<User> {
    query.filter(User::VIEWS.gt(100))
}

fn active(query: Select<User>) -> Select<User> {
    query.filter(User::VIEWS.gt(0))
}
```

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
// the model's entry
let popular = User::popular();
// the chain's method — the `UserScopes` trait the derive writes next to
// the model; generated code re-exports it from `crate::db::prelude`
let popular_and_active = User::popular().active();
```

Generated code writes a `prelude` module in `db/mod.rs`:
`use crate::db::prelude::*;` brings rok-data's prelude, every model, and
each model's `{Model}Scopes` trait. For hand-written models, re-export the
scope trait next to the model the same way (`pub use models::{User,
UserScopes};`) so callers need a single glob import.

A scope can take parameters — declared in the attribute, after the name,
in the same shape the entry takes them. The free function takes the query,
then the declared parameters; the entry and the chain method get the same
signature:

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
#[derive(Model)]
#[rok(scopes(older_than(n: i32) = older_than))]
struct User {
    id: i64,
    age: i32,
}

// the free function takes the query, then the declared parameters
fn older_than(query: Select<User>, n: i32) -> Select<User> {
    query.filter(User::AGE.gt(n))
}
```

<!-- source: crates/rok-data/tests/docs/migration_guide.rs -->
```rust
// the entry and the chain method take the same shape
let adults = User::older_than(18);
let seniors = User::query().older_than(65);
```

A generated model declares its scopes through the engine's `[scopes]`
configuration instead (`[scopes.users] famous = "crate::scopes::users::famous"`):
the engine wires `#[rok(scopes(famous = …))]` onto the row, and your crate
owns the functions the paths name.

Nothing changes for the scopes that already exist: the model's
`default_scope` (`#[rok(default_scope = …)]`) and `.unscoped()` keep their
meaning, and the closure-based `Select::scope` still takes a function.

### Write verbs moved to `ModelWrite`

The `Model` trait's write methods (`insert`, `insert_all`, `copy_in`, `upsert`,
`upsert_on`, `update`, `update_only`, `delete`, `force_delete`, `restore`)
are now in a separate `ModelWrite` trait. `Model` stays the read/query
surface; `ModelWrite` is the write surface. Both are in the prelude so
`use rok_data::prelude::*;` still brings both into scope. Generated models
implement both traits automatically. Hand-written models that implement
`Model` can also implement `ModelWrite` (defaults are provided).

The `Tracked` wrapper now requires `ModelWrite` (it uses `update_only` to
save only changed columns). Its `new` constructor and the model's `track()`
method now require the model to implement `ModelWrite` — which all derived
models do.

`row.rs` is now well under the 600-line gate.

### The conventions are inferred now

A model's own field names are the signal (workspace ADR 0015):

| The model has | It now gets | The opt-out |
| --- | --- | --- |
| `created_at` and `updated_at` fields | ORM-managed timestamps: `now()` on insert, `now()` on every update | `#[rok(no_timestamps)]` |
| a `deleted_at` field | soft deletes: the reads skip the rows where it is set, and `delete` marks the row instead of removing it | `#[rok(no_soft_delete)]` |

**This changes existing models.** A model with a `deleted_at` field that was
*not* a soft-delete column — an audit trail, a column that means something
else — now soft-deletes: its reads skip the rows where the column is set,
and its `delete` marks the row instead of removing it. Add
`#[rok(no_soft_delete)]` to such a model. The same goes for a model with
both `created_at` and `updated_at` fields that the application wrote
itself: it now gets `now()` written on insert and on every update. Add
`#[rok(no_timestamps)]` to keep the application's values.

The markers `#[rok(timestamps)]` and `#[rok(soft_delete)]` keep working and
mean the same thing — they are redundant now, not wrong. Carrying a marker
*and* its opt-out (`#[rok(timestamps)]` with `#[rok(no_timestamps)]`) is a
contradiction, and the derive refuses to compile it.

The generated rows change with it: the generator stops writing the markers
and writes the opt-out only where the database owns the column. Regenerate
and read the diff — a row that now carries `#[rok(no_timestamps)]` or
`#[rok(no_soft_delete)]` is a table whose columns the database fills.

### Validation errors carry paths; constraint violations read as field errors

`FieldError::field` is an owned path now (`String`, `"address.city"`,
`"items.2.sku"` for nested input), and a `FieldError` carries the rule's
`params`. Compare with `e.field.as_str()` / `e.field == "email"`;
`ValidationErrors::add` takes any `impl Into<String>`, so calls that pass
a literal still compile.

A unique or foreign-key violation on a row write (`insert`, `insert_all`,
`upsert`, `upsert_on`, `first_or_insert`, `update`, `update_only`) comes
back as `Error::Validation` with the column as the field and code
`unique` / `exists` — the same error a database rule reports (workspace
ADR 0021) — instead of `Error::Database`. `is_unique_violation()` /
`is_foreign_key_violation()` stay true for it; `constraint()` is `None`.
Code that matched `Error::Database` for these cases matches
`Error::Validation`, or keeps calling the two predicates. A violation on
columns the model has no field for, and any violation from the builders
(`insert_builder`, `update_all`, `Insert`), stays a database error.

### Generated validation errors are the runtime's

A project made from a template carries its own `src/error.rs`. After
regenerating, change its validation import and the `rok_data::Error`
mapping:

<!-- not compiled: a line-by-line edit of a scaffolded file -->
```rust,ignore
// before
use validator::ValidationErrors;
// after
use rok_data::ValidationErrors;

// before
rok_data::Error::Validation(_) => AppError::Validation {
    message: error.to_string(),
    details: None,
},
// after
rok_data::Error::Validation(details) => AppError::from(details),
```

A client reading a `422` sees the same `details` map. The difference is
that a rule's `params` no longer echo the rejected `value`.

### Generated inputs derive `rok_data::Input`

Regenerating replaces `validator` in the generated code (engine ADR
0025). In the project:

- **The manifest:** remove `validator` (and `regex`, if only rules used
  it). Add rok-data's `regex` feature when an `[api.rules]` rule is
  `regex`, `mobile` or `postalCode`; the generated `db/mod.rs` lists it.
- **Hand-written request types** that go through `ValidatedJson` derive
  `rok_data::Input` instead of `validator::Validate`. Wrap each rule in
  `rok(…)`: `length`, `range` and `email(message = …)` keep their syntax.

<!-- not compiled: a line-by-line edit of a hand-written type -->
```rust,ignore
// before
#[derive(serde::Deserialize, validator::Validate)]
pub struct Signup {
    #[validate(email(message = "email must be an email address"))]
    pub email: String,
}
// after
#[derive(serde::Deserialize, rok_data::Input)]
pub struct Signup {
    #[rok(validate(email(message = "email must be an email address")))]
    pub email: String,
}
```

- **Calling validation yourself:** `body.validate()` becomes
  `body.validate_fields()` (field rules), or `body.validate(&db).await`
  when the type has database rules.
- **`custom` functions:** a `custom` rule takes `fn(&T) -> Result<(),
  String>`. On an `Option` field it gets the value inside, and only for
  `Some`.

### Unchanged on purpose

- `filter` stays the canonical verb (the Diesel/SeaORM convention);
  `where_` is an alias, not a replacement.
- `all` stays next to `get`.
- `Tracked::save` keeps its name: a tracked record only ever writes the
  columns that changed, so it never has the insert-or-update ambiguity
  `Model::save` had.
- The `before_save` / `after_save` hooks keep their names (and their
  meaning: they run around the row-level update).
- `Model::update_all()` (the bulk set-builder) keeps its name.

## From `rok-db`

**Rust packages:** `rok-db` → `rok-data` (facade), `rok-db-core` →
`rok-data-core`, `rok-db-macros` → `rok-data-macros`, `rok-db-codegen` →
`rok-data-codegen`. Cargo dependency lines change accordingly. The
generated model crates pin `rok_data_version`/`rok_data_path` in
`rok-data.toml`'s `[gen]` table — those keys are the former
`rok_db_version`/`rok_db_path`.

**CLI:** `rok-db-gen generate|check|migrate …` →
`rok generate|rok generate check|rok migrate …`. Flag spellings and
defaults are preserved; `rok generate` groups `check` under itself, so
old scripts calling `rok-db-gen check …` need one extra word. Usage
errors now exit 2 (clap) where the old binary exited 1 for unknown
commands; command failures still exit 1.

**Journal:** `_rok_db_migrations` → `_rok_data_migrations`. Fresh
databases get the new name automatically. Existing databases must rename
the table once (`ALTER TABLE _rok_db_migrations RENAME TO
_rok_data_migrations`): the engine hard-codes its journal, so an
unrenamed database gets a fresh (`pending`-everything) journal and `up`
would re-run already-applied SQL. The rename is the safe path;
introspection deliberately ignores both journal names either way.

**MSRV:** the workspace is `rust-version = 1.94` everywhere (it already
matched the generated-code floor for SQLx 0.9).

## From `rok-sqlc`

**Rust package:** the engine is now the library `rok-data-sqlc`. The
standalone `rok-sqlc` binary is gone; every command lives behind the
workspace CLI:

```console
$ rok sqlc generate …   # was: rok-sqlc generate  (or: rok-sqlc g)
$ rok sqlc init …       # was: rok-sqlc init     (aliases g/i/a kept)
$ rok sqlc check-arch …
```

The grammar, options, help text, stdout formats and documented exit
codes are verbatim the engine's (its test suites now drive `rok sqlc …`
end to end). **In-scope exit-code change:** none for failing commands;
usage errors keep exiting 1, as they did for the standalone binary.

**File names / environments:**

- settings: `rok-sqlc.toml` → `rok-data-sqlc.toml` (same keys inside);
- env prefix: `ROK_SQLC_*` → `ROK_DATA_SQLC_*`
  (`ROK_DATA_SQLC_TEST_DATABASE_URL`, `ROK_DATA_SQLC_UPDATE_EXPECTED` …);
- journal of its API fixtures unchanged.

**Generated files:** first-line marker changes from
`@generated by rok-sqlc …` to `@generated by rok-data-sqlc …` on the
next regeneration (one line per file; `--check` reports the drift until
then). Everything that *recognizes* generated files — the writer,
`init --template`, the fixtures — accepts both markers, and templates
shipped in this workspace keep their old-marker files, so nothing must
be regenerated immediately.

**Dependency:** the engine crate keeps versions `sqlx 0.9`, actix-web 4,
utoipa 5 as before; only the package names changed.

## From `rok-fluent`

Superseded by the rok-db lineage (workspace ADR 0001). What carries over:

- **Schema introspection** → `rok_data_core::introspect`
  (`tables`, `columns`, `indexes`, `foreign_keys`, DDL dump). The old
  `SchemaInspector` calls map:
  `list_tables()` → `introspect::tables(&db)`,
  `table_columns(&t)` → `introspect::columns(&db, t)`,
  `table_indexes(&t)` → `introspect::indexes(&db, t)`,
  `table_foreign_keys(&t)` → `introspect::foreign_keys(&db, t)`.
- **Single-file migrations** (`-- up` / `-- down` sections) are still
  read by the unified loader — `rok migrate` accepts both formats side
  by side. New files created by `rok migrate create` are pairs, and the
  versioned `<NNNN_name>` file name is now required (a breaking change
  for unversioned rok-fluent names).
- The fluent `query.filter(…).order_by(…)` API maps to
  `rok-data-core`'s `Select`/`Expr` builder; there is no drop-in shim.

## From `rok-orm`

Superseded; the entity/repository vocabulary lives on in
`rok-data-core`'s `Expr`/`Select` and the `Model` derives. See the
mapping above; no shim is provided.

## Cross-cutting compatibility notes

| Area                 | Old (4 repos)                                   | rok-data                                              |
| -------------------- | ----------------------------------------------- | ----------------------------------------------------- |
| Binary               | `rok-db-gen`, `rok-sqlc`, `rok-fluent` CLI      | `rok` (groups above)                                   |
| Runtime dependency   | sqlx 0.9 (rok-db), none (rok-sqlc)              | sqlx 0.9, only inside `rok-data-core`                  |
| Journal table        | `_rok_db_migrations`                            | `_rok_data_migrations`                                 |
| Codegen config       | `rok-db.toml` `[gen] rok_db_*`                  | `rok-data.toml` `[gen] rok_data_*`                     |
| Engine config        | `rok-sqlc.toml`                                 | `rok-data-sqlc.toml`                                   |
| Env vars             | `ROK_*`                                         | `ROK_DATA_*`                                           |
| Generated headers    | `@generated by rok-sqlc`                        | `@generated by rok-data-sqlc` (legacy headers accepted everywhere generated files are recognized) |
| Usage-error exit     | 1 (both old binaries)                           | 2 at the root; 1 inside `rok sqlc …`                   |
| MSRV                 | 1.88 (rok-sqlc) / 1.94 (generated code)         | 1.94 everywhere                                        |
| License              | MIT OR Apache-2.0                               | unchanged                                              |
