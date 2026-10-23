# 07: ModelCollection without ceremony

**What to build:** The 0.4.0 headline call works as written: `let users = User::query().get(&db).await?; users.find(1); users.load(&db, &[User::POSTS])`. Fixes from the fae89e0 review:

- the derive's `where Self: Clone` on `to_collection` breaks every hand-written model without `Clone` (rustc rejects a trivially false bound) — make it compile for every model
- `get()` returns `ModelCollection<M>` (`all()` keeps `Vec<M>`); 0.3.0 is unreleased, so `get` is free to change
- `contains<K: IntoKey>` shadows `[M]::contains(&M)` through `Deref` → rename to `contains_key`
- `to_collection_vec(&[Self])` duplicates `From<Vec>`/`FromIterator` → remove
- `model_keys` and `to_query` handle composite keys; `to_query` never silently drops a row whose key fails to build
- `load_missing` reachable on the collection (spec story 4), not only on `Loaded<M>`
- `Error::Serialization` renamed (clashes with `is_serialization_failure`, SQLSTATE 40001) and given its predicate and constructor (workspace ADR 0007)

**Blocked by:** 01

**Status:** done

- [x] A hand-written non-`Clone` model compiles with the derive (compile test) — `to_collection`/`to_collection_vec` removed: they needed `Clone` and duplicated `From<Vec>`/`FromIterator` (and now `From<[M; N]>`); the doctests no longer derive `Clone`
- [x] `get()` → `ModelCollection`; spec example compiles as a doctest
- [x] `contains_key`, composite `model_keys`/`to_query`, collection `load_missing`
- [x] Tests for composite keys, `to_json`/`Serialize`, `partition` at the facade
- [x] `ModelCollection` entry in the glossary
