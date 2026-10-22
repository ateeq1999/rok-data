# 05: Documentation and migration guide for 0.3.0

**What to build:** The docs that ship with the breaking 0.3.0 release — updated getting-started guide, migration guide with before/after tables, CHANGELOG entry, and updated glossary/architecture docs. Every renamed call has its replacement documented so a developer can upgrade by following the guide.

**Blocked by:** 01, 02, 03, 04 (docs describe the final surface; must wait until all verbs/names are settled).

**Status:** ready-for-agent

- [ ] `docs/getting-started.md` rewritten around the new model-shaped verbs (`User::find`, `User::create`, `user.posts()`, `User::popular()`)
- [ ] `docs/migration-guide.md` gains a `## 0.3.0` section with before/after tables:
  - `User::query().filter(...).all(&db)` → `User::filter(...).get(&db)` (or keep both)
  - `users::read::get_by_id(&db, id)` → `User::find(&db, id)`
  - `users::write::create(&db, input)` → `User::create(&db, input)`
  - `users::write::update(&db, id, patch)` → `User::patch(&db, id, patch)`
  - `users::write::delete(&db, id)` → `User::destroy(&db, id)`
  - `user.save(&db)` → `user.update(&db)`
  - `User::create().set(...).exec(&db)` → `User::insert_builder().set(...).exec(&db)`
  - `relationships::load_posts(&db, &users)` → `user.posts().all(&db)` (after 02)
- [ ] Root `CHANGELOG.md` gains `## [0.3.0]` heading with the release narrative
- [ ] Root `GLOSSARY.md` gains **Model** (row type + verbs) and **Scope** (named reusable query fragment); engine `GLOSSARY.md` rewrites **Resource** and **Query chain** per spec D6/D3
- [ ] `docs/feature-matrix.md` and `docs/architecture.md` updated where they describe the data layer shape
- [ ] `docs/unified-pipeline-plan.md` gains the 0.3.0 phase entry
- [ ] `docs/cli-reference.md` unchanged (no CLI changes)