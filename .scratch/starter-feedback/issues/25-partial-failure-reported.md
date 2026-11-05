# 25: A failed `rok gen` step says what's done and what isn't

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** when a `rok gen` step fails (an App's generation, staging, the contract export, a Client's emission), `rok gen` ends with:
- the steps done
- the step that failed, with its error
- the steps skipped
- the command that finishes the rest (`rok gen`, once the cause is fixed)

Nothing is rolled back. `--json` carries the same steps.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] A failing `[contract] export` lists the data layer and migration as done and the Clients as skipped
- [x] `--json` has the steps
