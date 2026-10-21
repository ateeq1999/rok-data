# The generated model's shape: model-centric verbs, relations from foreign keys

Date: 2026-10-22 · Status: accepted

## Context

The engine writes one module per table — `read`, `write`, `filter`,
`relationships`, `listing` — and the model's row struct carries no verbs at
all. The verbs are reached through the module (`users::read::get_by_id(&db,
id)`), so a call site is a path the reader has to look up.

Relations are worse: they are declared **by hand** on hand-written models
(`#[rok(has_many(posts = Post::KEY))]`), and they are not declared at all on
generated ones. A generated `User` therefore has no `User::POSTS` and no
`user.posts()`, even though the schema says plainly that `posts.author_id`
references `users.id`. The generator reads that foreign key to build the
query layer, and then does not hand it to the model.

The workspace ADR 0014 decided the shape of the *runtime* surface (the verbs
live on the model). This ADR records the engine side of that decision: the
shape of what the engine writes.

## Decision

The generated model becomes the entry point, and the module keeps the
contracts:

- **The generated model carries the verbs** as inherent methods: the
  type-level ones (`query`, `filter`, `all`, `find`, `find_or_fail`,
  `find_many`, `count`, `exists`, `insert_builder`) and the row-level ones
  (`insert`, `update`, `upsert`, `upsert_on`, `delete`, `force_delete`,
  `restore`, `reload`) — see workspace ADR 0014 for the whole surface.
- **The generated model carries the write API**: `create(executor,
  NewUser)`, `patch(executor, id, UpdateUser)` and `destroy(executor, id)`,
  built on the engine's existing create-input and update-patch types. The
  module's read/write functions stay as thin wrappers over them for one
  release, so the API features and the hand-written service layer keep
  compiling.
- **Relations are derived from the schema's foreign keys.** A foreign key
  column becomes a `belongs_to` attribute on the referencing model, and the
  referenced table becomes a `has_many` on the other side, so a generated
  model has `User::POSTS` and `user.posts()` without the developer writing
  them. Where the foreign key does not name the relation unambiguously, the
  generator emits the explicit form (`belongs_to(author = User)`) rather than
  guessing.
- **Scopes are named** by path (`#[rok(scopes(popular = path::to::popular))]`),
  with a chainable model entry (`User::popular()`) and a chain method through
  the runtime's `Scope` trait, so scopes compose (`User::popular().active()`).
- **The module keeps what it is good at**: the filter/serde contracts, the
  search and listing code, the API-feature wiring and the OpenAPI document.
- **"Resource" is redefined**: from "the module the generator writes for one
  table" to "the API feature of one table", so that "model" and "resource"
  stop overlapping in the vocabulary.

The API features, the OpenAPI document, the filters' JSON shape and the patch
semantics are byte-compatible where they are not affected; the data layer
underneath them changes.

## Considered Options

- **Leave relations to the developer.** Rejected: the generator already reads
  the foreign key to build the query layer, so the information exists — making
  the developer restate it in the model is the duplication this ADR removes.
- **Put the relations in their own generated module** (a `relationships.rs`
  that already exists for the query layer). Rejected: navigation is from the
  model — `user.posts()` — and a relations module would keep the verbs in one
  place and out of the model's type, which is what workspace ADR 0014
  rejected.
- **Guess the relation name from any foreign key.** Rejected: a column named
  `owner_id` may point at `users` or at `teams`; where the name does not
  disambiguate, the explicit form is emitted.
- **Delete the module's read/write functions in the same release.** Rejected:
  the API features and the hand-written service layers call them; they stay
  as thin wrappers for one release and may be removed later.

## Consequences

- The engine's module-fixture corpus changes wholesale: a generated model
  now carries verbs, relations and scope entries. The corpus is refreshed
  through the sanctioned `ROK_DATA_SQLC_UPDATE_EXPECTED=1` runs.
- The generator's relation derivation becomes part of the generated output's
  contract: the `belongs_to`/`has_many` attributes it writes are read by the
  derive, so a generator/derive mismatch shows up as a compile error in the
  fixture gates rather than as a silent difference.
- The engine's glossary's **Resource** entry is rewritten to "the API feature
  of one table", and **Query chain** to the chain that starts at the model.
- The relation constants stay usable in the same chain as before: an include
  is one batched follow-up query, a relation that was not requested reads as
  absent, and soft-deleted related rows stay hidden (workspace ADR 0014).
