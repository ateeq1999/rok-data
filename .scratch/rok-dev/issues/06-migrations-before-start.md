# 06: Migrations before the API starts

Spec: `docs/specs/rok-dev.md` · workspace ADR 0034

**What to build:** for a Runnable with `migrate`, before starting it rok-dev checks the App's pending migrations through rok-data-cli's library (the project rule finds the App): `warn` prints them, `apply` runs them, `off` skips. A failed apply stops the start, naming the migration.

**Blocked by:** 04

**Status:** blocked

- [ ] `apply` migrates a fresh database, then starts (PostgreSQL test, skips without one)
- [ ] `warn` lists pending migrations and starts
