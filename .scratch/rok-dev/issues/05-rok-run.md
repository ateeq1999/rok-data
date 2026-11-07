# 05: `rok run <name>`

Spec: `docs/specs/rok-dev.md` · workspace ADR 0034

**What to build:** `rok run <name>` runs one Runnable in the foreground in its `dir`, with its `.env` loaded (the environment wins), without starting its dependencies, passing its exit code through.

**Blocked by:** 02

**Status:** blocked

- [ ] `.env` reaches the child; an exported variable wins (test)
- [ ] The exit code passes through
