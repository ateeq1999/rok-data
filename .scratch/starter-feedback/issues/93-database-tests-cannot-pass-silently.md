# 93: Database tests can't pass silently

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in the actix, axum, gRPC and Monorepo API Templates, `tests/support::Database::fresh`:

- loads the App's `.env` the way the App does (`config::load_env_file`), so a local `.env` just works;
- panics with "DATABASE_URL is required in CI" when `CI` is set and there is still no `DATABASE_URL`; locally it still skips with its note.

Karibu Pass's `cargo test --test client` reported every test `ok` in 0.00s without `DATABASE_URL` exported, though `apps/api/.env` had one (ticket 27).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] With `DATABASE_URL` only in `.env`, the database tests run (test)
- [ ] With `CI` set and no `DATABASE_URL`, they fail with that message
- [ ] The same in all four Templates
