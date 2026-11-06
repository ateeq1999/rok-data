# 07: The `grpc_template` gate and a rok-templates release

Spec: `docs/specs/grpc-api-template.md`

**What to build:** a `grpc_template` gate in rok-templates' `tests/gates/` like `axum_template`: init, generate, drift, `cargo clippy -D warnings`, `cargo test`. CI's gates matrix runs it. Then a rok-templates release carrying `grpc` in its catalog (the tag is the maintainer's).

**Blocked by:** 04, 05, 06

**Status:** done

- [x] Gate added and green locally and in CI
- [x] rok-templates release published with the `grpc` template
