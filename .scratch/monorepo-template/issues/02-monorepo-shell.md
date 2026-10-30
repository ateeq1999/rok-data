# 02: Monorepo shell — `--template monorepo`

**What to build:** `rok init --template monorepo` (and the same on the engine's own init grammar, tokens forwarded verbatim) scaffolds a full preview of the monorepo shape from ADR 0027, one App it serves today: a root `rok.toml` naming the monorepo's Apps, a root Cargo workspace wrapping them, and `apps/api/` holding the axum-api template's content reused **file for file** — same files, same destination behavior, same marker rules, under `apps/api/`. `template_drift` in rok-templates covers that reuse (a change to one copy that misses the other fails). `rok gen` from the monorepo root regenerates the api App's generated code. The other two Apps' directories simply don't ship yet — nothing half-working.

**Blocked by:** 01 (rok-templates owns the template catalog).

**Status:** ready-for-agent

- [ ] The catalog declares `monorepo` with its Apps: `api` first (web/mobile follow in later tickets); single-app templates keep working exactly as before
- [ ] Init writes: the root manifest files (workspace `Cargo.toml`, root `rok.toml` with the `[project]` shape naming the Apps) and the whole `apps/api/` App — files-if-absent policy holds across every App, single run, one report
- [ ] `apps/api/` reuses axum-api's files verbatim: the drift test that keeps the two single-app templates' shared files byte-identical is extended to keep `monorepo`'s api App identical to `axum-api`, file for file (paths remapped, CRLF sane), including fixups the axum template needs to live under `apps/api` (e.g. its manifest, its crate name scoping) — everything else drifts and fails
- [ ] `rok gen` at the monorepo root regenerates the api App (config resolution works through the root `rok.toml` naming the Apps), its drift modes intact
- [ ] The api App compiles and passes the checks its axum sibling already passes (compile gate extended to the monorepo layout); engine tests cover the scaffold inventory and the renaming into a project's crate name
- [ ] `docs/cli-reference.md` updated in the same change (the init contract rows for the new template value); format/clippy green; engine, rok-cli and rok-templates suites green
