# 60: A contract export that fails to compile says so

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** when `rok gen`'s contract export is a `cargo` command and it fails because the App doesn't compile, the failure summary (ticket 25) says: "the App doesn't build after this change: fix the call sites, then run `rok gen` to write the contract and the Clients", and shows the first compiler error lines. Other export failures keep today's message.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] A data-layer change that breaks a call site ends with that message and the first error
- [x] `--json` carries it in the failed step
