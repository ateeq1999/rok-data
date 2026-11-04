# 35: The web dev server listens on 127.0.0.1

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the Monorepo web App's `vite.config.ts` sets `server.host: "127.0.0.1"`, and the proxy target uses `127.0.0.1:8080`, matching the API's default bind, so scripts can use one address on every OS. The web README says how to change both.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] `curl http://127.0.0.1:3001` reaches the dev server on Windows
- [ ] README updated
