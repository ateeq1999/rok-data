# 12: The web App's route tree and Node

Spec: `docs/specs/starter-feedback.md`

**What to build:** the Monorepo web App pins `@tanstack/router-cli` to the router's version, with a `routes` script running `tsr generate`; the web gate runs it and fails if `routeTree.gen.ts` changes. The `test` script sets `NODE_OPTIONS=--no-experimental-webstorage`, and the gates pin Node.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] `bun run routes` regenerates the tree without a build
- [ ] The web tests pass on the pinned Node and on Node 26
