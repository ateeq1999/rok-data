# 50: Root scripts in the Monorepo

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it) · workspace ADR 0027

**What to build:** the Monorepo template's root gets a scripts-only `package.json` (`dev:web`, `dev:api`, `test:web`), with no workspaces and no JS monorepo tool, so nothing depends on the shell's current directory. `dev:api` names the API's package (`cargo run -p <api>`), and the root `Cargo.toml` sets `default-members = ["apps/api"]` so a bare `cargo run` at the root starts the API; the gates and the template CI keep `--workspace`, so `cargo test` still covers the desktop App and the Rust Client. The README shows them.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** ready-for-agent

- [ ] `bun run dev:web` from the root starts the web App
- [ ] No `workspaces` key; ADR 0027's "plain bun inside `apps/web`" holds
- [ ] A bare `cargo run` at the root starts the API; `cargo test --workspace` still covers every member
