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

## Considered Options

- **A separate gRPC generator beside the API generator**: a second model
  of the same Resources, which would drift from the HTTP one.
- **`protoc` from the system**: one more thing to install for every user
  and CI image.
- **`numeric` as `double`**: loses precision on money.
