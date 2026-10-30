# 09: Docs, upstream archive, finish

**What to build:** The branch reads like its ADR: the front-door contract (`docs/cli-reference.md`) shows `--template monorepo` and `--org` as contract text; getting-started grows the monorepo path; rok-templates' guide gains the monorepo page (Apps, one contract, gen loops, what's hidden and why) and the written push-notifications recipe (FCM) the ADR defers under ADR 0023; the crate's README no longer claims "files and nothing else". The docs index (`llms.txt`) and human map match the pages. The two upstream starter repos are marked archived with a pointer naming rok-templates as the only source of truth (the archive + README edit is an owner action — the ticket does everything short of the click). Everything green.

**Blocked by:** 07 (rust client) and 08 (renames) — docs describe the finished UX.

**Status:** ready-for-agent

- [ ] `docs/cli-reference.md`: init rows cover the monorepo value, `--org`, what a scaffold writes, regeneration paths — same PR as the grammar it contracts (already updated piecemeal in 02/08, verified complete here)
- [ ] `docs/getting-started.md`: the monorepo quickstart (init → api → web → mobile → regen) with compiled-sample discipline respected; the templates' guide page + recipes/ FCM (placeholder→real path) following the repo's working-page conventions
- [ ] rok-templates README + crate docs describe the catalog and the Apps; `llms.txt` and docs map refreshed in the same change
- [ ] Upstream pointers: both starter repos' READMEs prepared (with a pointer here), noted for archive; the vendored trees' own READMEs name rok-templates as home
- [ ] The whole suite is green for release-range purposes: fmt, clippy `-D warnings`, workspace tests, engine unit suite, rok-cli CLI suites, fixture gates (Rust + bun + flutter paths with their skip notes) locally where toolchains exist, plus a fresh monorepo scaffold end-to-end check (init all Apps, gen round-trip, api tests through the client)
