# 07: Generated Rust Client — clients/rust, proved against the api

**What to build:** `clients/rust`: a reqwest crate emitted from the same `openapi.json` reader, with its own types (callers do not compile with the server), a member of the root Cargo workspace next to `apps/api` (per ADR 0027). The api's integration tests call through the generated client, so client and server are proved to agree — the spec, the client and the server can't quietly disagree. A fresh scaffold gains `clients/rust` at init; `rok gen` regenerates it after re-exporting the artifact; generated-marker rules apply.

**Blocked by:** 04 (OpenAPI artifact + the subset model the readers share).

**Status:** ready-for-agent

- [ ] Emission covers the full utoipa subset the api's spec exercises, same stance as the TS/Dart emitters: unsupported constructs reported, not invented; one client with typed methods, own model types (serde), session/bearer handling by the spec, one consistent error type
- [ ] The emitted crate is its own package (no dependency on the server's types), compiles clean under the workspace's lint set treated as caller-gentle (its own reasonable lints, not the template's full severity), and sits in the root workspace with the api
- [ ] The api's integration suite calls through the generated client for the flows it already covers, so any client/server drift in practice fails those tests
- [ ] Init emits `clients/rust`; `rok gen` re-exports + re-emits; generation round-trips stable from a fixed spec
- [ ] `cargo fmt --all` clean, clippy `-D warnings` clean, workspace + fixture gates green (the api fixture now also proves the generated client compiles in the workspace)
