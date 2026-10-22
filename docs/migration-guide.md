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
| `row.save_only(&db, columns)` | `row.update_only(&db, columns)` | the row-level update of the listed columns only |
| `write::update(exec, id, input)` | `write::patch(exec, id, input)` | the generated update: returns the patched row, `None` when missing |
| `write::delete(exec, id)` | `write::destroy(exec, id)` | the generated delete: returns the deleted count (`u64`), not `bool` |

```rust
// before
let user = User::create()
    .set(User::EMAIL, "a@b.c")
    .exec(&db)
    .await?;
user.save(&db).await?;

// after
let user = User::insert_builder()
    .set(User::EMAIL, "a@b.c")
    .exec(&db)
    .await?;
user.update(&db).await?;
```

`save` is **removed, not aliased**: it is the one Eloquent verb whose
meaning is ambiguous (insert *or* update), and a non-optional typed key
leaves the runtime nothing to decide on. Use the explicit verbs — `insert`,
`update`, `upsert`, `upsert_on` — instead.

The generated data layer changes with the runtime, and its write helpers
take the Eloquent-shaped names: `update` becomes `patch` (the instance
verb `update` already means "write this row"), `delete` becomes `destroy`,
and `destroy` returns the number of deleted rows (`u64`), not a `bool`:
an `assert!(write::delete(...).await.unwrap())` becomes
`assert!(write::destroy(...).await.unwrap() > 0)`. The instance
`update`, `insert_builder()`, the query builders, `restore` and
`delete_permanently` keep their names. Regenerate and let the compiler
find the call sites.

### New verbs

| Verb                | Signature                                  | Notes |
| ------------------- | ------------------------------------------ | ----- |
| `Model::exists`     | `fn exists(executor) -> Result<bool>`      | does the table have a row? the model's own scope applies (soft deletes, default scope, tenant) |
| `Model::where_`    | `fn where_(expr) -> Select<Self>`         | the Eloquent spelling of `filter` |
| `Select::where_`   | `fn where_(expr) -> Self`                 | the Eloquent spelling of `filter`, on the chain |
| `Select::get`       | `fn get(executor) -> Result<Vec<Self>>`    | the Eloquent spelling of `all`; both stay |
| `Select::chunk`     | `fn chunk(executor, size, f) -> Result<()>` | the rows in windows of `size`, so a large table need not fit in memory |

```rust
// `where` is a Rust keyword, so only the raw identifier compiles; `filter`
// stays the canonical verb and the generated code keeps using it.
let admins = User::where_(User::ROLE.eq("admin")).all(&db).await?;

// one row or many, without fetching them
if User::exists(&db).await? {
    // …
}

// the same rows as `all`
let users = User::query().get(&db).await?;

// the rows in windows of 100, one query, every row once
User::query().chunk(&db, 100, |rows| async move {
    for user in rows {
        archive(&db, user).await?;
    }
    Ok(())
}).await?;
```

`chunk` streams the rows: only one window is in memory at a time, the
query runs once (not once per window), every matching row reaches the
callback exactly once in the order the query returns them, and the last
window is shorter when the rows don't divide by the size. The callback
returns a future, so a window can be written back to the database before
the next one arrives; the first error stops the walk and is returned.
The query holds its connection while the windows are processed, so a
callback that writes back needs a second one (give the pool more than one
connection).

### Named scopes

**New in 0.3.0.** A model can declare **named scopes**: reusable
constraints on a query chain, named by path. The derive gives the model an
entry (`User::popular()`) and the chain a method
(`User::popular().active()`), so scopes compose:

```rust
#[derive(Model)]
#[rok(scopes(popular = path::to::popular, active = path::to::active))]
struct User {
    id: i64,
    views: i32,
}

// a scope is a function from a query chain to a query chain
fn popular(query: &Select<User>) -> Select<User> {
    query.clone().filter(User::VIEWS.gt(100))
}

fn active(query: &Select<User>) -> Select<User> {
    query.clone().filter(User::VIEWS.gt(0))
}

// the model's entry
let popular = User::popular();
// the chain's method — the `UserScopes` trait the derive writes next to
// the model, so a call site brings it into scope with a `use`
let popular_and_active = User::popular().active();
```

Nothing changes for the scopes that already exist: the model's
`default_scope` (`#[rok(default_scope = …)]`) and `.unscoped()` keep their
meaning, and the closure-based `Select::scope` still takes a function.

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
