# 71: Small follow-ups from ticket 14

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:**
- **check-arch rule 4's message** adds that a pure helper with no feature state belongs in `shared/`. The rule itself is unchanged; the rule text in the templates' `AGENTS.md` waits for a rok-templates release, as ticket 42 found.
- **`rok gen`** prints `exporting the contract (building the App, slow the first time)…` before the contract export, so a long first build isn't silent.
- **The Rust Client's doc comment** for an operation whose sign-in is optional (`security((), ("bearer_auth" = []))`) says "sends the session's token when there is one".

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] The rule-4 violation message mentions `shared/` (test)
- [ ] The export line prints before the export (test)
- [ ] The optional-auth doc comment (emission test)
