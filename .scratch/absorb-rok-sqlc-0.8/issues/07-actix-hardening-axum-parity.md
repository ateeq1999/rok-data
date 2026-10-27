# 07: Actix hardening and axum parity (rok-sqlc 0.7)

**What to build:** The axum template gains the account flows (moved here from ticket 05, ported from upstream v0.7.0's `templates/axum-api`). The actix template hashes passwords on a blocking thread, its rate limit lives where upstream moved it, and its `/api/v1/auth` scope answers unknown paths and methods with the wire standard. Under axum, a malformed path or query answers `400 bad_request` with the wire wording. A repository test fails when a framework-independent template file differs between the two templates.

**Blocked by:** 05

**Status:** done

- [x] Ported behavior in both templates where it applies
- [x] Template-drift test in rok-cli's suites
- [x] Scaffolded `AGENTS.md` / `features/README.md` worded for `rok sqlc` and `rok.toml`

**How it shipped:**
- **actix (part 1, `a…` commit):** three-way merges, v0.6.0 → v0.7.0, of the auth routes and service, `security/password.rs` (`spawn_blocking`), `breach.rs` and `login.rs` tests, plus `rate_limit.rs` → `rate_limit/{mod,limiter}.rs`.
- **The axum template's account flows:**
  - **Framework-independent files:** copied from the workspace's actix template, so they carry the workspace's adaptations. That covers `queries/`, `shared/{security,mail}`, `limiter.rs`, the auth and account `dto.rs` and `service.rs`, `config.rs`, `openapi.rs`, `.env.example` and the schema truth.
  - **Framework files:** taken from upstream v0.7.0's axum template: handlers, routes, `rate_limit/mod.rs`, `error.rs` (plus the workspace's `From<rok_data::Error>`) and the tests.
  - **`app.rs`:** adapted. The generated `features::router(pool)` stays stateless, the hand-written routes are a `Router<AppState>` with `.with_state`, and a middleware puts `Arc<JwtKeys>` and the `PgPool` in the request extensions.
  - **`shared/auth.rs`:** rewritten generic over `S`, reading from the extensions, so one extractor serves both kinds of route.
  - **Migration:** `0002_auth` staged with `rok gen --migration`; it is identical to actix's.
  - **Verified against Postgres:** a project from `init --template axum` + `g` runs 103 tests (rate_limit, server and timeout included), none skipped.
- **Wire wording:** axum's `Path`/`Query` rejections are reworded as actix-web's; a path wiring bug is a `500`. The axum fixture pins both messages.
- **Drift test:** `crates/rok-cli/tests/template_drift.rs` checks `migrations/`, `schema/`, `queries/`, `shared/{security,mail}`, `limiter.rs`, the generated features' dto/model/repo/service_api, and the auth and account dto/service. A generated feature's scaffolded `service.rs` names the framework's wiring here, so it is left out.
- **`init` for axum:**
  - `rok sqlc init -w/--api-framework axum` and `rok init -w axum` scaffold the axum skeleton. Its files come from the axum template as it was before this ticket.
  - The flag conflicts with `--template`.
  - The CLI tests and a `fixture_compile` gate (`check-arch` and clippy-clean) cover it. That gate found a real bug: the data layer's Swagger UI hint always named `actix-web`; it now follows `api.framework`.
- **Wording:** the templates' AGENTS.md names the `rok-data-sqlc` marker, and both READMEs credit `rok sqlc`. Both still name `rok-data-sqlc.toml`, the config they ship, so "rok.toml" would be wrong there.
