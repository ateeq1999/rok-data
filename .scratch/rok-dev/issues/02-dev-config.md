# 02: `[dev]` settings in rok.toml

Spec: `docs/specs/rok-dev.md` · workspace ADR 0034

**What to build:** parsing and validation of `[dev.<name>]` tables: `run` (argv) or `compose`, `dir`, `ready` (`port:N`, `http:<path>`, `log:<regex>`), `depends`, `migrate` (`apply`/`warn`/`off`, default `warn`), `device`; each Runnable's `.env` from its `dir`. Unknown keys, `depends` cycles, a missing `run`/`compose` and an unknown Runnable in `depends` fail naming them. rok-data's project rule keeps ignoring `[dev]` (a test there).

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] Each `ready` form and every error case is unit-tested
- [ ] rok-data reads a rok.toml with `[dev]` unchanged (test in rok-data)
