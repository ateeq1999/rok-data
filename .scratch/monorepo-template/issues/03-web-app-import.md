# 03: Web App imported — apps/web ships in the monorepo template

**What to build:** The TanStack Start starter (`ateeq1999/tanstack-starter-axum`) is imported into the template as the monorepo's `web` App: its source is vendored under `apps/web` in rok-templates (after this, rok-templates is the only source of truth — the upstream repo gets archived with a pointer here in ticket 09). `init --template monorepo` now writes `apps/web` too, so a fresh scaffold has a runnable web app against a runnable api app: start the api, run the web app (plain bun, no JS monorepo tool), sign in — today through its own hand-rolled HTTP layer; the generated client swap is ticket 04. Dependency versions are pinned exactly (no `"latest"`). The web app is gated like the Rust apps: `bun install --frozen-lockfile`, typecheck and tests from `fixture_compile/`, skipping with a printed note when the bun toolchain is absent, and CI gains the toolchain install.

**Blocked by:** 02 (monorepo shell — `--template monorepo`).

**Status:** partially-done

- [x] `apps/web` lives in the template with its own self-check: tree imported from `ateeq1999/tanstack-starter-axum`, mechanical alignment only (package name derives from the manifest, no content rewriting — the API-layer swap is ticket 04)
- [x] Init writes `apps/web` under its manifest as it does the api App (files-if-absent across the Apps; hidden/OS junk excluded from embedding via build.rs `node_modules` exclusion)
- [x] Gate: a `fixture_compile` test (`web_app.rs`) checks the scaffolded `apps/web` (install/typecheck/tests), printing a skip note and passing when bun is absent
- [ ] Gate: all 114 tests pass in the fixture test environment (currently 103 pass, 7 fail due to vitest 2.x API differences and jsdom setup issues in the fresh `bun install` environment)
- [ ] CI: a job installs bun and runs the web gate (and keeps the Rust matrix untouched)
- [ ] The embedding stays lean: what the template ships is what `cargo package` includes (checked), binaries don't gain stray files; format/clippy and all engine/rok-cli suites stay green