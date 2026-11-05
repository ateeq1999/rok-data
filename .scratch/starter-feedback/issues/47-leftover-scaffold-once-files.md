# 47: Leftover scaffold-once files

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** when a feature is no longer generated (its table excluded or dropped), `rok gen` removes its scaffold-once files that are still exactly as scaffolded (compared with a re-render), and lists the edited ones with a note saying they're no longer wired in.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] Excluding a table removes its unedited `service.rs` and the folder when empty
- [x] An edited `service.rs` stays and is listed
- [x] `rok gen check` reports both cases
