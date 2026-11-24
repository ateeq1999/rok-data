# 07: `TOTP_ISSUER` setting

Spec: `docs/specs/starter-feedback.md`

**What to build:** the templates' TOTP issuer is a `TOTP_ISSUER` setting, defaulting to the project name without a trailing `-api`, documented in the config table and `.env.example`.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** done (rok-templates `spec/starter-feedback`, PR #19)

- [x] The default issuer for `karibupass-api` is `karibupass` (test)
- [x] `TOTP_ISSUER` overrides it
