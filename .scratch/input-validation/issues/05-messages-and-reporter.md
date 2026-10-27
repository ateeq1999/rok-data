# 05: Messages and the reporter

**What to build:** Per-rule `message = "… {min} …"`; a `Messages` provider keyed by rule code (and optionally field), passed at report time; an `ErrorReporter` trait whose default renders the generated APIs' `422 validation_failed` body (`details` by path).

**Blocked by:** 01

**Status:** done (2026-10-27) — see the notes at the end

- [x] Precedence: rule message > provider > built-in English
- [x] Default reporter body byte-identical to the generated APIs' (test against a template's answer)
- [x] axum / actix response integration behind the existing `web` support

**Done (2026-10-27).** Notes:
- The runtime's `axum` error responses moved to the wire standard's nested shape for every error, not only validation (breaking, CHANGELOG).
- **Open for ticket 08:** the generated APIs' `params` also echo the failing `value` (the `validator` crate's habit); input fields can be any type, so the derive carries the rule's arguments only. Ticket 08 decides whether the generated answers drop `value` (a wire change, documented) or the derive learns to echo values for the types that convert.
- The built-in English messages are the runtime's existing ones; ticket 07 aligns them with the engine's wording.
