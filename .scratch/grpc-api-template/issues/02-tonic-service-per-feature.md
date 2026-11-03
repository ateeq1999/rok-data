# 02: A tonic service per feature, over the service trait

Spec: `docs/specs/grpc-api-template.md` · engine ADR 0028

**What to build:** with `framework = "grpc"`, each feature gets a generated tonic service (instead of routes and handlers) that maps the request messages to the feature's DTOs, calls the same service trait (`service_api`) the HTTP handlers call, maps the results back, and maps errors to gRPC statuses (not found `NOT_FOUND`, validation `INVALID_ARGUMENT` with field errors in the details, unique violation `ALREADY_EXISTS`, credentials `UNAUTHENTICATED`, forbidden `PERMISSION_DENIED`, else `INTERNAL`). A new compile-gate fixture crate beside `tests/axum-app` is generated with `grpc`, builds its protos with `tonic-build` fed by `protox` (no `protoc`), and passes clippy `-D warnings`.

**Blocked by:** 01

**Status:** blocked

- [ ] The generated service calls the service trait; DTOs, `service_api` and `service.rs` are as for the HTTP frameworks
- [ ] The status mapping lives in one generated place
- [ ] Fixture crate `tests/grpc-app` (workspace-excluded, own `[workspace]`), gated like `fixture_compile/axum_app.rs`
- [ ] Its tests call the services over an in-process channel: create, get, list (page fields), update, delete, `NOT_FOUND`, `INVALID_ARGUMENT` (DB-backed parts skip with a printed note without a database URL)
- [ ] Drift check covers the generated services
