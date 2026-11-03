# Spec — The `grpc-api` Template and the engine's gRPC generation

Status: ready-for-agent · Implements workspace ADR 0030 (its gRPC half) and engine ADR 0028
Interview outcome in one line: gRPC is a third API framework (`framework = "grpc"`): per Resource the engine writes a `.proto` and a tonic service over the same service trait the HTTP handlers use; a `grpc-api` Template shares `axum-api`'s config, db, jobs and Capabilities, verifies the auth `axum-api` issues, and builds its protos with `protox` (no `protoc`).

## Problem Statement

A developer who wants a gRPC API from their schema has nothing to start from. rok generates HTTP APIs (actix-web, axum) from the schema directory, but gRPC means hand-writing every `.proto`, every message, every service and every mapping between rows and messages, and keeping all of it in step with the schema by hand. The services template of ADR 0030 needs exactly this as its building block, so it can't start either.

## Solution

`rok init my-api --template grpc` writes a tonic project, and `rok gen` generates it from the schema directory the same way it generates the HTTP templates. Each Resource gets a `.proto` file (its messages and a service with unary `Get`, `List`, `Create`, `Update` and `Delete`) and a generated tonic service that calls the same service trait as the HTTP handlers, so business logic a developer writes in a feature's `service.rs` serves gRPC unchanged.

The project shares `axum-api`'s configuration, data layer, jobs and Capabilities. It verifies the bearer tokens and API keys an `axum-api` issues, ships health and reflection, and includes one hand-written streaming example. Its protos are compiled at build time without `protoc` installed. A drift check fails CI when the schema and the generated `.proto` or service disagree.

## User Stories

1. As a backend developer, I want `rok init my-api --template grpc` to write a working tonic project, so that I start from a server that already builds, runs and passes its tests.
2. As a backend developer, I want `rok gen` to write one `.proto` per Resource from my schema, so that my API contract follows my tables without hand-writing messages.
3. As a backend developer, I want each Resource's service to have unary `Get`, `List`, `Create`, `Update` and `Delete` RPCs, so that the common operations exist without code.
4. As a backend developer, I want `List` to return the same page fields as the HTTP APIs (`items`, `total`, `page`, `per_page`), so that paging works the same over both transports.
5. As a backend developer, I want the generated gRPC service to call my feature's `service.rs`, so that the business rules I wrote for HTTP apply to gRPC too.
6. As a backend developer, I want nullable columns to become `optional` fields, so that "absent" and "zero" stay distinct on the wire.
7. As a backend developer, I want timestamps and dates to be `google.protobuf.Timestamp`, so that every gRPC client reads them as native time values.
8. As a backend developer, I want `numeric` columns to travel as strings, so that money and decimals never lose precision.
9. As a backend developer, I want my SQL enums to become proto enums with an `_UNSPECIFIED = 0` first value, so that the generated contract follows protobuf's rules.
10. As a backend developer, I want `jsonb` columns as `google.protobuf.Struct`, so that structured data stays structured.
11. As a backend developer, I want the generated `.proto` files marked `@generated` and placed under `proto/`, so that I know not to edit them.
12. As a backend developer, I want to write my own RPCs in hand-written `.proto` files that import the generated messages, so that custom operations reuse the generated types.
13. As a backend developer, I want `rok gen check` to fail when a generated `.proto` or service is out of date, so that CI catches a schema change nobody regenerated.
14. As a backend developer, I want the protos compiled at build time with no `protoc` installed, so that `cargo build` works on a fresh machine and in CI.
15. As a backend developer, I want an auth interceptor that accepts the bearer tokens and API keys my `axum-api` issues, so that one sign-in serves both APIs.
16. As a backend developer, I want a request without valid credentials to fail with `UNAUTHENTICATED`, so that clients get the standard gRPC status.
17. As a backend developer, I want login, registration, 2FA and passkeys to stay on HTTP, so that the template doesn't duplicate the auth flows.
18. As an operator, I want `grpc.health.v1` served, so that load balancers and Kubernetes probes can check the service.
19. As a developer, I want server reflection served, so that `grpcurl` and Postman list and call the services without the `.proto` files.
20. As a developer, I want one server-streaming example fed by `LISTEN/NOTIFY`, so that I have a working pattern for live updates.
21. As a backend developer, I want the template's config, database pool, migrations, jobs, mail, storage, telemetry and metrics to be `axum-api`'s, so that the two templates behave the same and a fix to one reaches both.
22. As a backend developer, I want a not-found row to be `NOT_FOUND`, a failed validation `INVALID_ARGUMENT` with the field errors, and a conflict `ALREADY_EXISTS`, so that the gRPC statuses mean what clients expect.
23. As a Rust client developer, I want the template's Rust client stubs from `tonic-build`, so that a Rust caller is typed end to end.
24. As a developer in another language, I want standard `.proto` files, so that I can generate a client with the stock protoc plugins for my language.
25. As a backend developer, I want `rok init --template grpc` listed beside `actix`, `axum` and `monorepo`, so that I find it where I find the others.
26. As a template maintainer, I want the `grpc-api` template held to the same gates as `axum-api` (it compiles, its generated files match generation, its tests pass), so that it can't silently rot.
27. As a template maintainer, I want the files `grpc-api` shares with `axum-api` held byte for byte by the drift check, so that the shared parts never fork.
28. As an engine maintainer, I want the gRPC output to come from the same Feature model the HTTP renderers use, so that an endpoint added to the model reaches every transport.

## Implementation Decisions

- **gRPC is a third API framework.** The engine's API configuration accepts `framework = "grpc"` beside `actix_web` and `axum`. The engine's one framework switch gains a gRPC arm. For gRPC, the per-feature routes and handlers are replaced by a generated tonic service; the feature's DTOs, service trait (`service_api`) and user-owned `service.rs` are generated as for the other frameworks, unchanged.
- **Per Resource, the engine writes:** a `proto/<resource>.proto` (`@generated`, proto3, a package per project, the row message, create and update input messages, the list request and page response, and a service with unary `Get`, `List`, `Create`, `Update`, `Delete`), and the feature's tonic service implementation. That implementation is `@generated` and calls the service trait, mapping messages to and from the DTOs.
- **Type mapping** (engine ADR 0028): text, `uuid`, `numeric` → `string`; `int2`/`int4` → `int32`; `int8` → `int64`; `float4` → `float`; `float8` → `double`; `bool` → `bool`; `bytea` → `bytes`; `timestamptz`, `timestamp`, `date` → `google.protobuf.Timestamp`; `jsonb`/`json` → `google.protobuf.Struct`; a nullable column → `optional`; a SQL enum → a proto enum whose first value is `<ENUM>_UNSPECIFIED = 0`.
- **Errors map to gRPC statuses** in one place: not found → `NOT_FOUND`; validation (with the field errors in the status details) → `INVALID_ARGUMENT`; unique violation → `ALREADY_EXISTS`; missing or bad credentials → `UNAUTHENTICATED`; forbidden → `PERMISSION_DENIED`; anything else → `INTERNAL`.
- **Build:** the template compiles `proto/` at build time with `tonic-build` fed by `protox` (a pure-Rust protobuf compiler), so no `protoc` is needed. The same build produces the Rust client stubs.
- **The `grpc-api` Template** lives in rok-templates beside `axum-api`. It shares config, db, migrations, jobs, mail, storage, telemetry, metrics, health and security as Shared files (the drift check holds them identical), and adds its own `app` wiring (the tonic server, interceptor, health, reflection) and the streaming example. `init --template grpc` selects it.
- **Auth:** an interceptor validates the bearer token (`axum-api`'s JWT, same secret and claims) or an API key on each call, and puts the caller where the service trait reads it, exactly as the HTTP extractor does. No auth RPCs.
- **Health and reflection** are built in (`tonic-health`, `tonic-reflection`); grpc-web is off.
- **Streaming:** one hand-written, scaffolded (user-owned) server-streaming RPC fed by `LISTEN/NOTIFY`; streaming is not generated.
- **Drift:** `rok gen check` covers the generated `.proto` files and services like every other generated file.
- **Engine ADR 0028** records the framework value, the file layout, the type mapping and the status mapping.

## Testing Decisions

- A good test drives the public seam and checks behaviour a user sees: the files generation writes, and the server's answers. It doesn't test private renderers or emission helpers.
- **Seam 1, the engine's output.** Generation unit tests over the shared fixture schema with `framework = "grpc"` assert the `.proto` content (messages, the mapped types, `optional`, enums with `_UNSPECIFIED`, the service's RPCs) and the tonic service's shape. Prior art: the engine's axum generation tests and the `api_openapi` snapshot.
- **Seam 1, compiled.** A new compile-gate fixture crate beside `tests/axum-app` (excluded from the workspace, with its own `[workspace]`) is generated with `framework = "grpc"`, then compiled with clippy `-D warnings` and tested. Its tests call the generated services over an in-process channel: create, get, list (page fields), update, delete, `NOT_FOUND`, `INVALID_ARGUMENT`. Prior art: `fixture_compile/axum_app.rs`.
- **Seam 2, the template.** rok-templates' gates gain a `grpc_template` gate like `axum_template`: init, generate, check drift, `cargo clippy` and `cargo test`. The template's own tests call the server in-process for CRUD, the interceptor (no token → `UNAUTHENTICATED`, a valid token → OK, an API key → OK), health and reflection. `template_drift` covers the files shared with `axum-api`.
- DB-backed tests skip with a printed note when no database URL is set, as everywhere.

## Out of Scope

- The `services` Template, the Gateway and Events (ADR 0030's second half): a later spec, which builds on this one.
- Serving gRPC from `axum-api` (a future `grpc` Integration, ADR 0029).
- Generated streaming RPCs, grpc-web, and gRPC clients in other languages (the stock protoc plugins cover those).
- Auth flows over gRPC (login, registration, 2FA, passkeys stay HTTP).
- `rok add` and Integration points (ADR 0029).

## Further Notes

- The spec spans two member repos: the engine change in rok-data, then the template in rok-templates, which needs a rok-data release whose generator knows `framework = "grpc"` (its gates build the generator at the pin in `rok-data.pin`).
- `protox` and `tonic-build` versions are pinned exactly in the template, like the template's other dependencies.
