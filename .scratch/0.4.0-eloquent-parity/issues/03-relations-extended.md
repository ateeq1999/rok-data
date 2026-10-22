# 03: Relationship extensions — has_many_through, has_one_through, belongs_to_many, where_has/doesnt_have, with_count/sum/avg/min/max/exists

**What to build:** Three new relation types emitted by the generator and available for hand-written models:
1. `HasManyThrough<P, C, Through>` / `HasOneThrough` — two-hop relations via intermediate model; `query(&self) -> Select<C>`, `load(&self, executor, &[P]) -> Many/One`.
2. `BelongsToMany<Target, Pivot>` — many-to-many via join table; `attach/detach/sync/sync_without_detaching/toggle/update_existing_pivot` on the relation; `attach`/`sync` accept optional pivot data.
3. Existence/aggregate scopes on `Select`: `where_has(relation, fn)`, `where_doesnt_have(relation, fn)`, `where_relation(relation, col, op, value)`; `with_count(relation)`, `with_sum(relation, col)`, `with_avg`, `with_min`, `with_max`, `with_exists` — these add `relation_count`, `relation_sum_col`, etc. to the selected parent model.
4. `ModelCollection::load_count(relation)` for deferred aggregate loading on fetched models.

**Blocked by:** 01 (ModelCollection for load_count), 02 (where_has uses subquery sugar).

**Status:** ready-for-agent

- [ ] New relation structs in `crates/rok-data-core/src/relation/through.rs` and `belongs_to_many.rs`
- [ ] Derive attributes: `#[rok(has_many_through(name = Final::FK, through = Through::FK))]`, `#[rok(has_one_through(...))]`, `#[rok(belongs_to_many(target = Target, pivot = pivot_table))]`
- [ ] Generator detects: FK chain A→B→C → `has_many_through`; join table (two FKs + unique) → `belongs_to_many` with pivot methods
- [ ] `Select::where_has/where_doesnt_have/where_relation` using subquery `EXISTS`/`NOT EXISTS`
- [ ] `Select::with_count/sum/avg/min/max/exists` adding subquery columns to projection
- [ ] `ModelCollection::load_count` runs batched aggregate query
- [ ] Facade tests for each relation type and aggregate
- [ ] Module fixtures for generated pivot methods and through relations