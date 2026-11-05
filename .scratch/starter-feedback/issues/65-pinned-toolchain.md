# 65: A pinned toolchain in every template

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** every template ships `rust-toolchain.toml` pinning the stable channel the gates test (with `clippy` and `rustfmt`), so a new lint reaches a project only when a rok-templates release moves the pin. `rust-version` stays the declared minimum. rok-templates' CI runs the gates on the pinned toolchain, keeps its minimum-version check, and a test checks the pin matches what CI installs.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] Each Template (and the Monorepo root) has `rust-toolchain.toml`
- [ ] CI's gates use the pinned toolchain; the pin and CI agree (test)
- [ ] The template READMEs say how to move it
