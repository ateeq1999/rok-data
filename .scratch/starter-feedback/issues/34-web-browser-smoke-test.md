# 34: A browser smoke test in the web App

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the Monorepo web App gains Playwright, pinned exactly, with one smoke flow: sign in with the dev seed's account, land on home, sign out. `bun run e2e` runs it against a running API. The web gate runs it in CI with the API and Postgres up, and skips it locally with a note when no API is reachable.

**Blocked by:** 11 (the dev seed), grpc-api-template/08 (the gRPC Template shipped)

**Status:** ready-for-agent

- [ ] The smoke flow passes in the web gate in CI
- [ ] `bun run e2e` documented in the web README
