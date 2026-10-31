# 07: Generated Rust Client — clients/rust, proved against the api

**What to build:** `clients/rust`: a reqwest crate emitted from the same `openapi.json` reader, with its own types (callers do not compile with the server), a member of the root Cargo workspace next to `apps/api` (per ADR 0027). The api's integration tests call through the generated client, so client and server are proved to agree — the spec, the client and the server can't quietly disagree. A fresh scaffold gains `clients/rust` at init; `rok gen` regenerates it after re-exporting the artifact; generated-marker rules apply.

**Blocked by:** 04 (OpenAPI artifact + the subset model the readers share).

**Status:** done

- [x] Emission covers the full utoipa subset the api's spec exercises, same stance as the TS/Dart emitters: unsupported constructs reported, not invented; one client with typed methods, own model types (serde), session/bearer handling by the spec, one consistent error type
- [x] The emitted crate is its own package (no dependency on the server's types), compiles clean under the workspace's lint set treated as caller-gentle (its own reasonable lints, not the template's full severity), and sits in the root workspace with the api
- [x] The api's integration suite calls through the generated client for the flows it already covers, so any client/server drift in practice fails those tests
- [x] Init emits `clients/rust`; `rok gen` re-exports + re-emits; generation round-trips stable from a fixed spec
- [x] `cargo fmt --all` clean, clippy `-D warnings` clean, workspace + fixture gates green (the api fixture now also proves the generated client compiles in the workspace)

## Comments

**Where things live.** The emitter is `clients::rust` in the engine, beside `clients::ts`, reading the same `clients::openapi` model and reusing the TS Client's operation naming (`ts::name_operations`, in `snake_case`). `[clients] rust = "<dir>"` in `rok.toml` selects it; `clients::generate` writes it with the same three modes and marker rules. Engine ADR 0027 records the shape.

**The crate.** The engine emits `src/{lib,runtime,types,calls}.rs` and `README.md`, every file `@generated`, rustfmt-clean as emitted. `Cargo.toml` is *not* emitted: the template scaffolds `clients/rust/Cargo.toml.template` once (package `api-client`, reqwest 0.12 with `json` + `rustls-tls`, serde, serde_json; its own lints: `unsafe_code` forbidden, `missing_docs` and the panicking shortcuts warned). Judgement call: the manifest stays the user's to tune, and the template keeps no nested `Cargo.toml` that `cargo package` would drop. It is a root workspace member; the template's `Cargo.lock` gained only the `api-client` entry. The api App dev-depends on it (`path = "../../clients/rust"`).

**Shape.** `Client::new(base_url)`; `client.<tag>().<operation>(path params, body, query)`, async on reqwest. `set_token` (shared by clones) sends the bearer token only to operations carrying the bearer marker; `on_unauthorized(handler)` runs on a `401` to such a call, unless made through `keep_session()`; `with_header` for an `X-API-Key` or a raw body's content type. One `ApiError` (`status`, `code`, `message`, `details`/`field(path)`; status 0 for transport, `decode_error` for an undocumented answer). Types: formats read past (`uuid`/`date-time` are `String`); an inline object shaped like a schema reuses it (page items are `Vec<UserResponse>`); `allOf` of objects merges; `oneOf` is an untagged enum (`LoginResponse::{TokenResponse, PendingToken}`). Judgement call: a property that may be left out *and* be `null` is `Option<Option<_>>` only in types a request body reaches (keep vs clear), a plain `Option` in answers.

**Client-driven api tests.** `apps/api/tests/client/` (one test binary, 11 tests) starts the API on a real socket (`serve_as(AppRole::Web, ...)`, `/auth` rate limit off) and drives, through the generated client: sign-up/verify/sign-in/profile edit (tri-state clear), a `422`'s field details, the 401/`on_unauthorized`/`keep_session` rules, password change/forgot/reset, admin users list/get/update/remove, admin-only refusals, invitations with the audit log and the job queue, API keys (bearer and `X-API-Key`, revoke), two-factor setup/enable/recovery-code sign-in/disable, media upload/metadata/content/list/remove, and the profile photo. Judgement call: these are added beside the router-driven suites, which stay (they check what the client hides: headers, rows, rate limits).

**Gates.** `fixture_compile/monorepo_template.rs`: the drift check covers `clients/rust`'s sources (refreshed only via `ROK_DATA_SQLC_UPDATE_EXPECTED=1`); `cargo test -p axum-api` includes `tests/client/` (with `DATABASE_URL` from `ROK_DATA_SQLC_TEST_DATABASE_URL` when set); the new `monorepo_rust_client_passes_its_checks` runs `cargo fmt --check`, `clippy --all-targets --locked -D warnings` and `test --locked` on `api-client` in a fresh scaffold. `gen_clients.rs` covers `rok gen`/check for `rust` and init's emission.

**Limits.** No `Box` for a self-referential schema (none in the spec; one would not compile); an array or object query parameter (which serde_urlencoded cannot send; none in the spec) is refused at emission, naming the operation and parameter. The desktop App will link reqwest 0.12 through the client beside rok-ui's 0.13.
