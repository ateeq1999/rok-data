---
name: rok-data-models
description: >
  Authoring and evolving model code with the rok-data runtime. Use when:
  writing or editing `#[derive(Model)]` structs, choosing per-column or
  per-model attributes (`generated`, `primary_key`, relations, scopes,
  tenant, soft_delete, version, validate), calling the Model read chain
  or `ModelWrite` verbs, working with relations (`has`, `where_has`,
  `with_count`, `with`, through), generated models' inputs
  (`NewXxx`/`UpdateXxx`, `create`/`patch`/`destroy`), typed column
  constants and casts, or deciding hand-written vs generated models.
  Fires for any task that writes Rust against a rok-data data layer.
---

## Decide the surface first

- **Generated** (engine wrote it, e.g. `src/db/<table>/*.rs`): models,
  inputs, chains, API features all in place — never hand-edit; regenerate.
  One import brings everything:
  `use crate::db::prelude::*;`
- **Hand-written**: your crate, your derive. The facade gives one import:
  `use rok_data::prelude::*;`

## The derive, by attribute

```rust
#[derive(Debug, Clone, Model)]
#[rok(table("note"), per_page = 20, tenant, soft_delete, version)]
#[rok(has_many(notes = Note::AUTHOR_ID))]       // the FK column's const
#[rok(belongs_to(author = User))]               // relation name = owner model
#[rok(scopes(popular = popular))]               // entry: Note::popular()
struct Note {
    #[rok(generated)]                           // DB fills it; excluded on insert
    id: i64,
    #[rok(primary_key)]                         // alternate key column(s)
    slug: String,
    #[rok(column("created_at"))]                // name override
    created: OffsetDateTime,
    #[rok(skip)]                                // not persisted (loaded view fine)
    author_name: String,
}
```

Other accepted attributes: `#[rok(validate(unique("email")))]`,
`#[rok(validate(non_empty))]` / `length(min…)` / `range(min…)` (input
validation with database rules — workspace ADR 0021,
`docs/decisions/0021-…md`; spec in `docs/specs/input-validation.md`),
`#[rok(default_scope(...))]`,
`#[rok(nested)]`, `#[rok(type_name(...))]`, value objects
(`value_object` + `set_with`) and `Encrypted` columns. Unknown meaning →
check `crates/rok-data-macros` and `cargo doc -p rok-data`, never guess.

**Key types (0.4.0):** `Model` carries `type Key`; `find` /
`find_or_fail` / `find_many` take `impl Into<Self::Key>` — the key
field's type, or a tuple of key fields in field order for composite keys.
`Product::find(&db, category_id)` does not compile: wrong key type.

## Reads: the chain

```rust
let note = Note::find_or_fail(&db, note_id).await?;
let most_read = Note::filter(Note::VIEWS.gt(100))
    .order_by(Note::VIEWS.desc())
    .all(&db).await?;

let found = Note::query()
    .filter_any([Note::TEXT], |column| column.ilike("%rust%"))
    .filter(Note::VIEWS.gt_column(Note::AUTHOR_ID))
    .get(&db).await?;

let writers = User::query().has(User::NOTES).all(&db).await?;
let view = User::query()
    .with_count(User::NOTES)
    .with(User::NOTES.with(Note::AUTHOR))
    .all(&db).await?;
let how_many = view.count(User::NOTES, &user);
let notes = view.get(User::NOTES, &user)?;

let author = note.author().first(&db).await?;    // accessor starts a chain
let popular = Note::popular().all(&db).await?;   // scope entry
```

Columns are *typed constants* (`Note::VIEWS`) — conditions and casts are
per-column (`gt`, `gt_column`, `ilike`, in/eq/ne/like, `asc()`/`desc()`,
date parts like `.year()`). Paging verbs sit on the same chain:
`paginate`, `page`, `simple_paginate`, `cursor_paginate`, `chunk_by_id`,
`lazy_by_id` — [`docs/pagination.md`](../../../docs/pagination.md) is the
authoritative table.

## Writes: `ModelWrite`

```rust
let stored = Note { id: 0, author_id: user_id, text: "d".into(), views: 0 }
    .insert(&db).await?;
stored.update(&db).await?;                        // whole row, by key
stored.update_only(&db, [Note::TEXT, Note::VIEWS]).await?;
if let Some(note) = found.first() { note.increment(&db, Note::VIEWS, 1).await?; }
stored.delete(&db).await?;
```

Generated models carry the generated verbs, taking the generated inputs
(`None` = unchanged, `Some(None)` = NULL):

```rust
let supplier = Supplier::create(&db, NewSupplier { … }).await?;
Supplier::patch(&db, supplier.id, patch).await?;
Supplier::destroy(&db, supplier.id).await?;      // NotFound when absent
// soft-delete pair: restore / delete_permanently
// join tables: SomeRole::attach / detach / sync
```

One "not found" convention: key-targeting verbs fail with
`Error::NotFound` (`is_not_found()`); optional lookups stay `Option`.

## Verifying a change

1. `cargo fmt --all && cargo clippy --workspace --all-targets -D warnings`.
2. DB-touching tests use `#[rok_data::test]` (schema-scoped `Db`); they
   honestly skip without `DATABASE_URL` — the skip is printed, never silent.
3. User-facing Rust surface in `docs/` is **compiled**: a new sample goes
   into `crates/rok-data/tests/docs/*.rs` first, then into the page with
   a matching `<!-- source: … -->` marker. Samples in
   `getting-started/migration-guide/pagination` must not drift from it.
