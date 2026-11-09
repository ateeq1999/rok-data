# 09: `rok port list` and `rok port free`

Spec: `docs/specs/rok-dev.md` · workspace ADR 0034

**What to build:** `rok port list [PORT…]` shows the process (pid, name, command line) holding each `[dev]` port or each given one; `rok port free <PORT>…` stops it and its process tree after asking, or with `--yes`, and refuses a process it can't identify. Windows and Unix.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] list and free against a helper holding a port, on Windows and Unix (tests)
- [ ] Asks before stopping; `--yes` skips the question
