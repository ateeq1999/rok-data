# 01: `framework = "grpc"` writes a `.proto` per Resource

Spec: `docs/specs/grpc-api-template.md` · engine ADR 0028

**What to build:** the engine accepts `framework = "grpc"` in the API configuration. With it, `rok gen` (and `rok sqlc generate`) writes `proto/<resource>.proto` for every Resource: proto3, a package per project, the row message, create and update input messages, the list request and page response (the `Page` fields: items, total, page, per_page), and a service with unary `Get`, `List`, `Create`, `Update`, `Delete`. Types follow engine ADR 0028. The files carry the `@generated` header and `rok gen check` reports them when stale.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `framework = "grpc"` parses; an unknown framework still fails with the valid values (now including `grpc`)
- [x] One `.proto` per Resource under `proto/`, with the `@generated` header
- [x] Type mapping per engine ADR 0028, nullable → `optional`, enums with `<ENUM>_UNSPECIFIED = 0` first, `google/protobuf/timestamp.proto` and `struct.proto` imported only when used
- [x] The service holds the 5 unary RPCs; `List` returns the page message
- [x] Generation unit tests over the shared fixture schema assert the above
- [x] `rok gen check` fails on a hand-edited or stale `.proto`
