# 05: Messages and the reporter

**What to build:** Per-rule `message = "… {min} …"`; a `Messages` provider keyed by rule code (and optionally field), passed at report time; an `ErrorReporter` trait whose default renders the generated APIs' `422 validation_failed` body (`details` by path).

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] Precedence: rule message > provider > built-in English
- [ ] Default reporter body byte-identical to the generated APIs' (test against a template's answer)
- [ ] axum / actix response integration behind the existing `web` support
