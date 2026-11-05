# 20: `rok gen` fails on a query it can't type

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** `rok gen` (and `rok gen check`) run the engine strictly by default: a query whose result column can't be typed fails generation with the file, the query, the expression and the `-- column: <name> <RustType>` hint. `--lenient` restores skipping with a warning; `--strict` stays accepted as a no-op alias. The `--json` body carries the same failure.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `expires_at <= now() AS expired` fails `rok gen` naming the query and the hint
- [x] `--lenient` skips it with the warning, as before
- [x] CLI reference, changelog and migration guide (a behaviour change) updated
