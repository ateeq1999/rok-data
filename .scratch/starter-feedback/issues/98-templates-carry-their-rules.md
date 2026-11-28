# 98: The templates' hand-written bodies carry their rules into the contract

Spec: `docs/specs/starter-feedback.md` (Typed web data) · engine ADR 0027 (amended 2026-11-28)

**What to build** (in rok-templates):
- **Login stops hinting at the password rule.** `CredentialsDto` splits in two:
  - `RegisterDto`: email, and an 8–128 character password.
  - `LoginDto`: email, and any non-empty password.

  A wrong-length login answers "invalid email or password", not "password must be 8 to 128 characters". Do this in axum, actix, gRPC and the Monorepo API, wherever `CredentialsDto` serves both.
- **Every hand-written body and query-parameter struct carries its rules.** Each template's hand-written `ApiDoc` registers ticket 97's modifier, so auth, two-factor, users, api-keys, media and the rest put their `#[rok(validate(…))]` rules, messages and transforms into the contract.
- **Regenerate:** the Monorepo's contract (`apps/api/openapi.json`) and its TypeScript, Dart and Rust Clients.

**Blocked by:** a rok release carrying ticket 97

**Status:** blocked

- [ ] A login with a 7-character password answers as a wrong password does (test)
- [ ] The Monorepo contract's `RegisterDto.password` has `minLength: 8`, `maxLength: 128` and its message; `email` has `format: email`
- [ ] The gates pass
