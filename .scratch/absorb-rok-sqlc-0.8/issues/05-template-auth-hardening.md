# 05: Template auth hardening (rok-sqlc 0.6)

**What to build:** Scaffolded projects (actix and axum) get upstream 0.6's auth: `AuthUser` checks the database on every request; a versioned `auth` migration; `JWT_SECRET` under 32 bytes stops startup; wiring through `app::configure(cfg, &AppState)`.

**Blocked by:** dx-first 03

**Status:** ready-for-agent

- [ ] Both templates; generated code uses the model verbs
- [ ] Template integration tests (auth, login, sign-up, account) ported
- [ ] Fixture compile gates green; CHANGELOG breaking entries (env var, migration, wiring)
