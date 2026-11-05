# 53: rust-analyzer builds in its own folder

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** every template ships `.vscode/settings.json` (scaffolded once, the user's) with `"rust-analyzer.cargo.targetDir": true`, so the editor builds in `target/rust-analyzer` and never competes with `cargo test` for the build lock. The template READMEs say how to do the same in other editors.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] Each Template carries the settings file; the Monorepo at its root
- [ ] README line for other editors
