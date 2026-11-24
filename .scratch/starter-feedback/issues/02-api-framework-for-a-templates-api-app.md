# 02: `-w` picks a Template's API App framework

Spec: `docs/specs/starter-feedback.md`

**What to build:** `rok init kp -t monorepo -w actix_web` (or `axum`) scaffolds the Monorepo with that framework's API App, drawn from the matching single-app Template, so no variant is kept by hand twice; the default stays `axum`. `-w` with a single-app Template is an error naming the Template to use instead.

**gRPC is left out of the Monorepo** (decided 2026-11-24). `-t monorepo -w grpc` is a usage error. It says the Monorepo's Apps talk to the API through its OpenAPI contract, and points to `-t grpc` for a gRPC API on its own.

**Order:** the rok-templates catalog work below comes first, for axum and actix. rok-data's side follows once a rok-templates release carries it. Box 2 and the gRPC refusal can be built in rok-data now.

**Blocked by:** grpc-api-template/08 (done)

**Status:** ready-for-agent (rok-templates catalog first; see the order above)

- [ ] The Monorepo scaffolds with axum's and actix's API App, and each combination passes the gates
- [ ] `-t monorepo -w grpc` is refused, pointing to `-t grpc`
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
- **gRPC:** `grpc-api` serves no OpenAPI document, but the Monorepo's contract and its TS, Dart and Rust Clients are generated from one. Decided: gRPC is left out of the Monorepo (see the top of this ticket).
- **What rok-data can do afterwards:** let `-w` combine with `-t`. Today `crates/rok-cli/src/cli/mod.rs` declares `conflicts_with = "template"`.
