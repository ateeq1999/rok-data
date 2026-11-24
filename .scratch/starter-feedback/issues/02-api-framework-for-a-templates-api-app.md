# 02: `-w` picks a Template's API App framework

Spec: `docs/specs/starter-feedback.md`

**What to build:** `rok init kp -t monorepo -w actix_web` (or `axum`, `grpc`) scaffolds the Monorepo with that framework's API App, drawn from the matching single-app Template, so no variant is kept by hand twice; the default stays `axum`. `-w` with a single-app Template is an error naming the Template to use instead.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** needs-triage (rok-templates work and a gRPC decision come first; see below)

- [ ] The Monorepo scaffolds with each framework's API App, and each combination passes the gates
- [ ] `-w` with `-t axum` (and the other single-app Templates) says why, naming the Template to use
- [ ] CLI reference, examples and guide updated

## What an agent found (2026-11-24)

- **The copy today:** `templates/monorepo/apps/api` is a hand-kept copy of `templates/axum-api`. Only `Cargo.toml.template` and `Dockerfile` differ between the files they share.
  - The Monorepo copy drops `.dockerignore`, `.github/workflows/ci.yml` and `Cargo.lock`.
  - It adds `Dockerfile.dockerignore`, `examples/openapi.rs`, `openapi.json`, `tests/openapi_artifact.rs` and `tests/client/*`.
- **The catalog:** rok-data reads Templates only through `rok_templates::catalog::files(name)`, one directory per Template. It can't say "the files of Template X, plus these Monorepo-only files". rok-data takes rok-templates 0.7.0 from crates.io, so any new catalog API needs a rok-templates release first.
- **Needed in rok-templates:**
  1. A catalog API that builds the Monorepo's `api` App from a framework's single-app Template: that Template's files, minus a list of excluded files, plus the Monorepo-only files.
  2. Monorepo-only files per framework, including the manifest edits and the Dockerfile. actix has no `examples/openapi.rs`, no `tests/openapi_artifact.rs` and no `tests/client`.
  3. `rok.toml`'s `[contract].export` follows the framework; today it is hard-coded to `-p axum-api`.
  4. The Monorepo gate runs once per framework.
- **Decision needed: gRPC.** `grpc-api` serves no OpenAPI document, but the Monorepo's contract and its TS, Dart and Rust Clients are generated from one. What do the web, mobile and desktop Apps talk to when the API is gRPC? Options:
  - leave `-w grpc` out of the Monorepo;
  - add a REST gateway;
  - generate the Clients from the proto files.
- **What rok-data can do afterwards:** let `-w` combine with `-t`. Today `crates/rok-cli/src/cli/mod.rs` declares `conflicts_with = "template"`. The single-app error (box 2) could be built now, but how it reads depends on the above.
