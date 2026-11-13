# 32: Constraint violations say which constraint

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in the templates' `error.rs` (axum, actix, grpc and the Monorepo's API), `AppError::Conflict(String)` becomes `Conflict { kind, constraint, message }`, where `kind` is `Unique` or `ForeignKey` and `constraint` is `Option<String>`. `From<sqlx::Error>` fills them; both stay 409 (gRPC: `ALREADY_EXISTS`). An `err.constraint()` accessor lets a feature map a constraint to its own error.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** ready-for-agent

- [ ] A unique and a foreign-key violation carry their kind and constraint name (tests)
- [ ] The HTTP status and error body are unchanged (contract tests pass)
- [ ] The Shared files stay identical across templates
