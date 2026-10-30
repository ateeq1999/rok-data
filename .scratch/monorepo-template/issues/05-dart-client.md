# 05: Generated Dart Client

**What to build:** `clients/dart`: a typed pub package emitted from the same `openapi.json` reader, shaped for the mobile ticket that will consume it (06): typed calls for every path, models for the schemas, session credential handling consistent with the api's auth (bearer/cookie as the spec defines), and the same error surface per operation (validation issues surfaced, not swallowed). A fresh scaffold gains `clients/dart` at init; `rok gen` regenerates it with the api artifact; generated-marker rules apply. Verifiable standalone: emission is unit-tested against the api's spec, and the emitted package itself passes its own static analysis where a toolchain exists (this ticket does not bring the Flutter app — that is 06).

**Blocked by:** 04 (OpenAPI artifact + the subset model the readers share).

**Status:** ready-for-agent

- [ ] Emission covers the whole utoipa subset the api's spec exercises: paths, methods, schemas/oneOf where emitted, request/response bodies, statuses; unsupported constructs are reported, not invented
- [ ] The emitted package is idiomatic enough for a Flutter consumer: one client class with typed methods, model classes, a session credential provider hook, one consistent error type carrying status + validation issues
- [ ] Init emits `clients/dart`; `rok gen` regenerates it after re-exporting the api artifact; generated markers carry on the files so re-emission policy matches the other Clients
- [ ] Emission is snapshot/round-trip tested: regenerating from the same spec is stable; changing the spec changes the client in the expected places
- [ ] The emitted package passes static analysis when the toolchain is present (skip-with-note otherwise), so ticket 06 starts from a sound package; format/clippy and all Rust suites green
