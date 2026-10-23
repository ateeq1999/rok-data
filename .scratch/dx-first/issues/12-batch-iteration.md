# 12: Batch iteration — chunk, chunk_by_id, lazy, each

**What to build:** Laravel's "walk a big table" verbs, safe under concurrent writes:

| Laravel | rok-data |
|---|---|
| `chunk(100, fn)` | `.chunk(&db, 100, f)` (exists; offset-based) |
| `chunkById(100, fn)` | `.chunk_by_id(&db, 100, f)` — keyset on the primary key, safe when `f` updates or deletes rows |
| `lazy()` / `lazyById()` / `lazyByIdDesc()` | `.lazy(&db, 100)` / `.lazy_by_id(&db, 100)` / `.lazy_by_id_desc(..)` → a `Stream<Item = Result<M>>` fetched in chunks |
| `each(fn)` / `eachById(fn)` | `.each(&db, f)` / `.each_by_id(&db, f)` over the lazy stream |

The callback DX must work with a pool: today's `chunk` example (`async move { archive(&db, rows) }` inside an `FnMut`) does not compile. The callback receives the rows and can use `&db` without the user restructuring ownership; returning `Ok(false)` (or a `ControlFlow`) stops early like Laravel's `return false`.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] `chunk_by_id`, `lazy`, `lazy_by_id`, `lazy_by_id_desc`, `each`, `each_by_id`
- [ ] Test: a `chunk_by_id` callback that deletes its rows still visits every row exactly once (offset `chunk` would skip)
- [ ] Early-stop test for chunk and each
- [ ] The documented chunk example compiles (doctest, not `ignore`)
