# 04: `rok dev`: readiness, reuse and order

Spec: `docs/specs/rok-dev.md` · workspace ADR 0034

**What to build:** `rok dev [NAME…]` starts the named Runnables and their dependencies (all, when none are named). Every build runs at once; a process starts when its `depends` are ready; long builds print progress lines. A Runnable whose `ready` already passes is reused and left running on exit. `http:` readiness polls with a generous per-request timeout and never aborts an in-flight response; `log:` watches the Runnable's own output.

**Blocked by:** 03

**Status:** blocked

- [ ] Order, reuse and readiness against helper binaries (one slow to answer HTTP)
- [ ] A failing Runnable stops the rest and names it
