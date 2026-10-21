# ADR 0014: the model-shaped data layer — the verbs live on the model

Date: 2026-10-22 · Status: accepted

## Context

The data layer is fluent, but it is not *legible* to a developer who knows
SQL and knows Eloquent. Three things make a call site cost a moment of
hesitation:

- **The verbs are on the wrong object.** A read is `users::read::get_by_id(&db,
  id)` and a write is `users::write::create_one(&db, input)` — the model is a
  passive row struct, and the verbs live in sibling modules, so every call
  site is a path the reader has to look up.
- **The vocabulary is invented rather than shared.** The where-clause verb is
  `filter` while the engine's own `XFilter` struct (a typed search input)
  already owns the word "filter": two different things, one word. And there is
  no named scope, no `Model::create([...])`-shaped typed create, and no
  relation on a generated model at all — a generated `User` has no `User::POSTS`
  and no `user.posts()`, because relations are declared by hand on hand-written
  models only.
- **The write path is one column at a time**, and the conventions (managed
  timestamps, soft deletes) must be *declared* per model, even though the
  column names already say which convention applies.

The friction is not in one place; it is in the shape of every call.

## Decision

The data layer becomes **model-shaped**: the model is the object you call, and
the module keeps what it is good at.

- **Reads and query entry points live on the model** as inherent methods:
  `User::find(&db, id)`, `User::all(&db)`, `User::count(&db)`,
  `User::filter(expr)`, `User::query()`. The static verbs are `query`,
  `filter`, `all`, `find`, `find_or_fail`, `find_many`, `count`, `exists` and
  `insert_builder`; the row-level verbs are `insert`, `update`, `upsert`,
  `upsert_on`, `delete`, `force_delete`, `restore` and `reload`.
- **The write path is typed**: the generated create input (`NewUser`) and
  update patch (`UpdateUser`) become the model's own write API —
  `User::create(&db, input)`, `User::patch(&db, id, patch)`,
  `User::destroy(&db, id)` — and the insert builder stays the low-level path
  (`User::insert_builder()`).
- **Scopes are named**: a scope is a function from a query chain to a query
  chain, declared by path, with a chainable model entry (`User::popular()`)
  and a chain method so scopes compose.
- **The module keeps the contracts**: the filter/serde contracts, the search
  and listing code, the API-feature wiring and the OpenAPI document stay
  where they are, and the module's read/write functions remain as thin
  wrappers over the model's methods for one release.
- **"Resource" is redefined**: from "the module the generator writes for one
  table" to "the API feature of one table", so that "model" and "resource"
  stop overlapping.

The release is **0.3.0 and breaking**: the runtime's public surface and the
generated output both change. `where` stays spelled `filter` (with `where_`
as an alias) because `where` is a Rust keyword — see
[ADR 0015](0015-inferred-conventions.md) for the conventions half of this
release and the engine's ADR 0018 for the generated code's shape.

## Considered Options

- **Keep the module-centric surface and add model-shaped wrappers beside
  it.** Rejected: two ways to say the same thing is how the vocabulary got
  invented in the first place; a reader still has to learn which surface is
  canonical.
- **Stage the change with deprecations** (keep `save`, keep the old verb
  names for a release). Rejected: it doubles the surface area and makes every
  project regenerate twice for no user benefit; the framework is pre-1.0 and
  `docs/migration-guide.md` exists for exactly this.
- **Hard-rename `filter` to `where`.** Rejected for a language reason:
  `fn where(..)` does not compile, and the raw identifier (`r#where`) reads
  badly in a chain, so the alias is spelled `where_`. The
  canonical verb stays `filter` (the Diesel/SeaORM convention) and `where_`
  is offered as an alias for Eloquent muscle memory.
- **Keep `save()` as "insert or update".** Rejected: Eloquent's `save()`
  decides by whether the key is set, and our keys are non-optional typed
  values, so the decision would need a sentinel. The explicit verbs
  (`insert`, `update`, `upsert`) replace it — `save` is removed, not aliased.

## Consequences

- The generated code's shape changes: a generated model carries the verbs, the
  relation attributes and the scope entries. The engine's module-fixture
  corpus is refreshed accordingly (through the sanctioned
  `ROK_DATA_SQLC_UPDATE_EXPECTED=1` runs, never by hand).
- The runtime's public surface changes in one release: `create()` (the insert
  builder) is renamed `insert_builder()`, `save()` is removed in favour of
  `update()`, and `exists()`, `get()`, `chunk()` and `where_` are added.
- `docs/migration-guide.md` carries the 0.3.0 before/after for every renamed
  call; the root glossary gains **Model** and **Scope**; the engine glossary's
  **Resource** entry is rewritten.
- The API features, the OpenAPI document and the filters' JSON shape are
  byte-compatible where they are not affected — the data layer underneath them
  changes.
- The legacy command surfaces (`rok generate`, `rok sqlc …`) are not touched.
