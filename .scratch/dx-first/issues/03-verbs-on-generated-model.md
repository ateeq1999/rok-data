# 03: The write verbs live on the generated model

**What to build:** What 0.3.0 spec D5 and engine ADR 0018 promised and ticket 0.3.0/01 marked done but did not ship: `Book::create(&db, input)`, `Book::patch(&db, id, patch)`, `Book::destroy(&db, id)` as inherent methods on the generated model. Today the generator writes them as free functions with full bodies (`books::write::create(&db, input)`) and no `impl Book` exists. The module functions become thin wrappers over the model methods (kept for one release, deprecated), and the generated API services call the model verbs.

**Blocked by:** 02

**Status:** done

- [x] Generator emits an inherent impl on each model with `create`, `patch`, `destroy` (and the permanent-delete variant where soft delete applies)
- [x] Module functions delegate to the model methods and carry `#[deprecated]` pointing at them
- [x] Generated services call `Model::verb`, not `repo::verb`
- [x] Generated code uses `Book::filter(..)` / `Book::find(..)` instead of `Book::query().filter(Book::ID.eq(id))`
- [x] D4: the generator writes only opt-outs — no redundant `#[rok(primary_key)]` on `id`, no `#[rok(table = "...")]` matching the convention
- [x] `examples/api` hand-written code uses the model surface (0.3.0 maintainer story 5) — its services make no data-layer calls (they keep the generated defaults); its tests call `Author::create`, `Book::create`, `Loan::create`
- [x] Snapshots, module corpus and fixture compile gates refreshed and green
