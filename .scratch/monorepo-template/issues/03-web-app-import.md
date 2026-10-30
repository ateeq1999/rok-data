# 03: Web App imported — apps/web ships in the monorepo template

**What to build:** The TanStack Start starter (`ateeq1999/tanstack-starter-axum`) is imported into the template as the monorepo's `web` App: its source is vendored under `apps/web` in rok-templates (after this, rok-templates is the only source of truth — the upstream repo gets archived with a pointer here in ticket 09). `init --template monorepo` now writes `apps/web` too, so a fresh scaffold has a runnable web app against a runnable api app: start the api, run the web app (plain bun, no JS monorepo tool), sign in — today through its own hand-rolled HTTP layer; the generated client swap is ticket 04. Dependency versions are pinned exactly (no `"latest"`). The web app is gated like the Rust apps: `bun install --frozen-lockfile`, typecheck and tests from `fixture_compile/`, skipping with a printed note when the bun toolchain is absent, and CI gains the toolchain install.

**Blocked by:** 02 (monorepo shell — `--template monorepo`).

**Status:** ready-for-agent

- [ ] `apps/web` lives in the template with its own self-check: tree imported, mechanical alignment only (package name derives from the manifest, no content rewriting — the API-layer swap is not this ticket), and `bun install --frozen-lockfile && bun run typecheck && bun test` passes on the imported tree
- [ ] Init writes `apps/web` under its manifest as it does the api App (files-if-absent across the Apps; hidden/OS junk excluded from embedding), and the web app runs against `apps/api`'s locally-served spec: sign-in works at the enforced URL contract (base URL/env pointed at the local api, auth flows calling the axum endpoints it already serves)
- [ ] Gate: a `fixture_compile` test checks the scaffolded `apps/web` (install/typecheck/tests), printing a skip note and passing when bun is absent; commits gate the sourced tree the same way
- [ ] CI: a job installs bun and runs the web gate (and keeps the Rust matrix untouched)
- [ ] The embedding stays lean: what the template ships is what `cargo package` includes (checked), binaries don't gain stray files; format/clippy and all engine/rok-cli suites stay green
