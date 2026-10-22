# 01: ModelCollection — in-memory collection methods

**What to build:** A `ModelCollection<M>` wrapper around `Vec<M>` with `Deref<Target = Vec<M>>` that provides the high-value Laravel Collection methods: `find`, `find_or_fail`, `model_keys`, `load`, `load_missing`, `unique`, `to_query`, `contains`, `partition`, `intersect`, `diff`, `except`, `only`, `fresh`, `to_json`, plus `Serialize` impl. Generated models get `fn to_collection(&self) -> ModelCollection<Self>` and `fn to_collection_vec(models: &[Self]) -> ModelCollection<Self>`.

**Blocked by:** None (can start immediately — 0.3.0 runtime is stable).

**Status:** ready-for-agent

- [ ] `ModelCollection<M>` struct in `crates/rok-data-core/src/collection.rs` with `Deref<Target = Vec<M>>`
- [ ] Methods: `find(key) -> Option<&M>`, `find_or_fail(key) -> Result<&M>`, `model_keys() -> Vec<PK>`, `load(&mut executor, &[Relation]) -> Result<Loaded<M>>`, `load_missing(...)`, `unique() -> Self`, `to_query() -> Select<M>`, `contains(key) -> bool`, `partition(predicate) -> (Self, Self)`, `intersect(other) -> Self`, `diff(other) -> Self`, `except(keys) -> Self`, `only(keys) -> Self`, `fresh(&executor) -> Result<Self>`, `to_json() -> String`
- [ ] `impl<M: Serialize> Serialize for ModelCollection<M>` (delegates to inner `Vec`)
- [ ] Derive emits `to_collection` / `to_collection_vec` on generated models
- [ ] Facade tests: SQL-shape + DB-backed for each method
- [ ] Module fixture expectations updated