# 03: Relationship extensions — has_many_through, has_one_through, belongs_to_many, where_has/doesnt_have, with_count/sum/avg/min/max/exists

**What to build:** Three new relation types emitted by the generator and available for hand-written models:
1. `HasManyThrough<P, C, Through>` / `HasOneThrough` — two-hop relations via intermediate model; `query(&self) -> Select<C>`, `load(&self, executor, &[P]) -> Many/One`.
2. `BelongsToMany<Target, Pivot>` — many-to-many via join table; `attach/detach/sync/sync_without_detaching/toggle/update_existing_pivot` on the relation; `attach`/`sync` accept optional pivot data.
3. Existence/aggregate scopes on `Select`: `where_has(relation, fn)`, `where_doesnt_have(relation, fn)`, `where_relation(relation, col, op, value)`; `with_count(relation)`, `with_sum(relation, col)`, `with_avg`, `with_min`, `with_max`, `with_exists` — these add `relation_count`, `relation_sum_col`, etc. to the selected parent model.
4. `ModelCollection::load_count(relation)` for deferred aggregate loading on fetched models.

**Blocked by:** 01 (ModelCollection for load_count), 02 (where_has uses subquery sugar).

**Status:** done (2026-10-27) — three items changed by decision, below

- [x] New relation structs in `crates/rok-data-core/src/relation/through.rs` (done: `HasManyThrough`, `HasOneThrough`; `Relation` gained overridable `batch_filter` / `batch_sql` / `correlate` / `owner_of_target` so a relation describes its own batch) and `belongs_to_many.rs` (`BelongsToMany` with `attach` / `attach_with` / `detach` / `detach_all` / `sync` / `sync_without_detaching` / `toggle` / `update_existing_pivot`)
- [x] Derive attributes: `#[rok(has_many_through(name = Final::FK, through = Through::FK))]`, `#[rok(has_one_through(...))]`, `#[rok(belongs_to_many(name = Target, pivot = "table"))]` (the spec's `target = …` became the relation's name, like `has_many`; key columns default to `{owner}_id` / `{target}_id`, `foreign_key` / `related_key` override)
- [x] Generator detects — **changed by decision:** an FK chain A→B→C is not turned into `has_many_through` automatically (every chain would become a relation, named by guesswork); through relations are declared. A join table pairing two primary keys (the schema's existing many-to-many inference) → `belongs_to_many` on both sides
- [x] `Select::where_has/where_doesnt_have/where_relation` using subquery `EXISTS`/`NOT EXISTS`, plus `has(rel)` / `doesnt_have(rel)`
- [x] Aggregates — **changed by decision:** not columns on the parent (a Rust struct has no field for them), but batched includes parked on the loaded view: `with_count` / `with_exists` on the chain, `User::POSTS.sum::<i64>(Post::VIEWS)` / `avg` / `min` / `max` handles loaded with `with`, read with `view.count` / `view.exists` / `view.aggregate(handle, row)`
- [x] `ModelCollection::load_count` runs batched aggregate query
- [x] Facade tests: `tests/features/{has,through,many_to_many}.rs`
- [x] Module fixtures: the axum-app fixture's `notes` / `labels` models and `milestone-1.txt` refreshed (sanctioned run); through relations are declared, so no fixture emits them