# 37: `rok` loads the App's `.env` for its database commands

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** `rok migrate`, `rok db`, `rok schema` and `rok gen`'s database steps load `<app>/.env` for the App they act on (found by ticket 24's rule).
- **Precedence:** a variable already in the environment wins.
- **Reporting:** the command's config line says `loaded apps/api/.env`.
- **Opting out:** `--no-env-file` skips it.

**Blocked by:** 24

**Status:** blocked

- [ ] `rok migrate up` at a Monorepo root uses `apps/api/.env`'s `DATABASE_URL`
- [ ] An exported `DATABASE_URL` overrides the file's
- [ ] `--no-env-file` reads nothing
