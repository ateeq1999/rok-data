# 50: Root scripts in the Monorepo

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it) · workspace ADR 0027

**What to build:** the Monorepo template's root gets a scripts-only `package.json` (`dev:web`, `dev:api`, `test:web`), with no workspaces and no JS monorepo tool, so nothing depends on the shell's current directory. The README shows them.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] `bun run dev:web` from the root starts the web App
- [ ] No `workspaces` key; ADR 0027's "plain bun inside `apps/web`" holds
