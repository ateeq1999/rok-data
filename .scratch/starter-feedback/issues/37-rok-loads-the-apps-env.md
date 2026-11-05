# 37: `rok` loads the App's `.env` for its database commands

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** `rok migrate`, `rok db`, `rok schema` and `rok gen`'s database steps load `<app>/.env` for the App they act on (found by ticket 24's rule).
- **Precedence:** a variable already in the environment wins.
- **Reporting:** the command's config line says `loaded apps/api/.env`.
- **Opting out:** `--no-env-file` skips it.

**Blocked by:** 24

**Status:** done

- [x] `rok migrate up` at a Monorepo root uses `apps/api/.env`'s `DATABASE_URL`
- [x] An exported `DATABASE_URL` overrides the file's
- [x] `--no-env-file` reads nothing

**Note (gen):** `rok gen` and `rok gen check` read no database today (staging a migration diffs the truth files against the snapshot), so there is no database step to load the `.env` for. When one is added, call `commands::env_file::load(env, Some(&target))` for its target and append `, loaded <file>` to its `using` line, as `commands::place::migrations_dir` does.
