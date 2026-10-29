# ADR 0023: capabilities ship built in, switched by the environment

Date: 2026-10-29 · Status: accepted

A template's capabilities (logs and traces, metrics, jobs, crons,
security headers, rate limits, and so on) are always in the template and
compiled in, each inert until an environment variable turns it on, as
`MAIL_TRANSPORT`, `STORAGE_URL` and `BREACH_CHECK` already work.
Integrations that pick a vendor (Sentry, Redis, an identity provider) are
written recipes, not code in the template. One code path is what keeps
the templates testable and `template_drift` byte-for-byte.

## Considered Options

- **Init-time selection** (`init --template axum --with otel,jobs`): a 2ⁿ
  matrix of projects to compile and test, and the two templates could no
  longer be compared byte for byte.
- **A post-init `add` command** (`rok sqlc add jobs`): it would have to
  patch `app.rs`, `config.rs` and `Cargo.toml` after the user owns them.

## Consequences

- Every project compiles the OpenTelemetry and metrics crates, even with
  them switched off.
- Plain `rok sqlc init` emits no capabilities, as engine ADR 0020 keeps auth
  template-only.
