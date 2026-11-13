# 02: `-w` picks a Template's API App framework

Spec: `docs/specs/starter-feedback.md`

**What to build:** `rok init kp -t monorepo -w actix_web` (or `axum`, `grpc`) scaffolds the Monorepo with that framework's API App, drawn from the matching single-app Template, so no variant is kept by hand twice; the default stays `axum`. `-w` with a single-app Template is an error naming the Template to use instead.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** ready-for-agent

- [ ] The Monorepo scaffolds with each framework's API App, and each combination passes the gates
- [ ] `-w` with `-t axum` (and the other single-app Templates) says why, naming the Template to use
- [ ] CLI reference, examples and guide updated
