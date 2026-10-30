# 02: the data layer on the generated Rust client

**Blocked by:** 01, and ADR 0027's ticket 07 (`clients/rust`)
**Status:** blocked

Spec: What ships (the data layer).

The repositories call `clients/rust`; the starter's hand-written HTTP calls
go. The client's validation errors still reach form fields. `clients/rust` is
a member of the monorepo's root workspace.

Done when no hand-written HTTP call is left in `apps/desktop`, and a change to
the API's contract that `rok gen` regenerates into the client breaks or flows
through the desktop App's build.
