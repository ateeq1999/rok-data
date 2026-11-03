# 11: Local setup that works on the first run

Spec: `docs/specs/starter-feedback.md`

**What to build:**
- **`.env`:** the template APIs load `.env` with dotenvy in debug builds only.
- **Dev seed:** a `seed` binary creates a verified admin and a verified ordinary user and prints their credentials. `REQUIRE_VERIFIED_EMAIL` stays `true`.
- **Postgres port:** `compose.yaml` maps `"${POSTGRES_PORT:-5432}:5432"`, and `.env.example` sets `POSTGRES_PORT` and uses it.
- **Stale name:** the Monorepo API's stale `actix_api` database name is fixed.

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] Debug `cargo run` reads `.env`; release does not
- [ ] The seeded accounts sign in (test against the gates' database)
- [ ] A non-default `POSTGRES_PORT` works end to end
