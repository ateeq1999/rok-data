# ADR 0028: gRPC is a third API framework

Date: 2026-11-03 · Status: accepted (implements workspace ADR 0030's generated-code shape)

## Decision

- `framework = "grpc"` joins `actix_web` and `axum`. The framework switch
  that chooses the routes and handlers chooses, for gRPC, a generated
  tonic service per feature instead. The feature's DTOs, service trait
  and user-owned `service.rs` are the same for every framework, so one
  service serves every transport.
- Per Resource: `proto/<resource>.proto` (`@generated`, proto3; the row
  message, create and update inputs, the list request and page; a service
  with unary `Get`, `List`, `Create`, `Update`, `Delete`) and the
  feature's tonic service (`@generated`), which maps messages to the DTOs
  and calls the service trait. `rok gen check` covers both.
- Types: text, `uuid` and `numeric` are `string` (`numeric` stays exact);
  `int2`/`int4` `int32`, `int8` `int64`, `float4` `float`, `float8`
  `double`, `bool` `bool`, `bytea` `bytes`; `timestamptz`, `timestamp`
  and `date` `google.protobuf.Timestamp`; `json`/`jsonb`
  `google.protobuf.Struct`. A nullable column is `optional`. A SQL enum
  is a proto enum whose first value is `<ENUM>_UNSPECIFIED = 0`, as
  proto3 requires.
- Statuses: not found `NOT_FOUND`; a validation failure
  `INVALID_ARGUMENT`, its field errors in the details; a unique violation
  `ALREADY_EXISTS`; credentials `UNAUTHENTICATED`; forbidden
  `PERMISSION_DENIED`; anything else `INTERNAL`.
- Projects compile `proto/` with `tonic-build` fed by `protox`, so no
  `protoc` is installed anywhere.

- Settled with the first slice: `proto/` sits at the project's root
  (beside the `src/` holding `api.output`); the package is the crate's
  name with `.v1` (`shop_api.v1`); an SQL enum is nested in the row
  message, since every Resource shares the package; the types this ADR
  doesn't name (times of day, intervals, money, ranges, network
  addresses) travel as `string`, an array as `repeated`; every update
  field is `optional`.
- Settled with the service slice: a feature's tonic service is its
  `grpc.rs`, implemented on the feature's own service struct (the
  server's trait imported as `<Feature>Rpc`, the service trait's methods
  called by path, since both name `list`, `create`, `update`). What goes
  for gRPC: `routes.rs`, `handlers.rs`, the shared `health.rs` and
  `tenant.rs`, the wire standard's error body, the data layer's
  extractor and the OpenAPI document (`openapi = true` fails). What
  stays: `dto.rs`, `service_api.rs`, `service.rs`, `model.rs`,
  `repo.rs`, and `pagination.rs`, `response.rs` and `validation.rs`
  (`FieldFailures`) in `shared/`. The root's `generated.rs` includes the
  compiled package (`proto`) and holds `routes(pool)`, a tonic `Routes`.
- The one status mapping is `impl From<AppError> for Status` in the
  generated `shared/grpc.rs`, matched on `AppError::code()` so a variant
  the user adds falls to `INTERNAL` rather than breaking the build. A
  conflict is `ALREADY_EXISTS` whether unique or foreign key (`AppError`
  doesn't tell them apart), and so is a validation failure whose every
  field error is `unique` (the generated pre-check reports a taken value
  as one). The codes the ADR doesn't name: a malformed request
  `INVALID_ARGUMENT`, a stale version `ABORTED`, a missing version
  `FAILED_PRECONDITION`, a body over the limit `RESOURCE_EXHAUSTED`. The
  field errors travel as `google.rpc.BadRequest` violations
  (`tonic-types`).
- The caller and the tenant are read from the request's extensions, where
  the template's interceptor puts an `AuthUser` and the tenant value: no
  caller is `UNAUTHENTICATED`, a non-admin on an admin feature
  `PERMISSION_DENIED`, no tenant `INTERNAL` (a wiring bug).
- Fields convert through two traits in `shared/grpc.rs`, `FromWire` and
  `IntoWire`, with impls for only the types the messages carry: typed ids
  through their inner value, an SQL enum by its variants' order (the
  proto's numbers), a time through `Timestamp` (a date at midnight UTC),
  JSON through `Struct` (a non-object value is `INTERNAL` when written).
  A message has no `null`: an absent update field keeps the column, as an
  absent HTTP field does, so gRPC never clears a nullable column; an
  empty repeated field is no value. A versioned row's update message
  carries the version as its last field, so the columns keep their
  numbers. A list paged by cursor is refused: `List` pages by number.
  The field numbers still follow the column order, so a column added
  before others renumbers them — a wire-compatibility concern left open.

## Considered Options

- **A separate gRPC generator beside the API generator**: a second model
  of the same Resources, which would drift from the HTTP one.
- **`protoc` from the system**: one more thing to install for every user
  and CI image.
- **`numeric` as `double`**: loses precision on money.
