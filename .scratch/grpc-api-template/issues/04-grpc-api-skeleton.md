# 04: The `grpc-api` Template skeleton

Spec: `docs/specs/grpc-api-template.md` · workspace ADR 0030

**What to build:** in rok-templates, a `grpc-api` template: the files it shares with `axum-api` (config, db, migrations, jobs, mail, storage, telemetry, metrics, health, security) held identical by `template_drift`, its own tonic server wiring, `proto/` compiled at build time with `tonic-build` fed by `protox`, `tonic-health` and `tonic-reflection`, and its API generated with `framework = "grpc"` from the template's schema. A catalog entry named `grpc`. The template's own tests call its services in-process for CRUD.

**Blocked by:** 03

**Status:** blocked

- [ ] Catalog entry `grpc` → `grpc-api`
- [ ] Shared files byte-identical to `axum-api`'s (`template_drift`)
- [ ] Builds with no `protoc`; dependencies pinned exactly
- [ ] Health and reflection served; tested
- [ ] Generated CRUD tested in-process
