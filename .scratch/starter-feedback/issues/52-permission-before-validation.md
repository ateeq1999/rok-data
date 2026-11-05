# 52: Permission before validation

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** a `check-arch` rule: a handler whose body extractor (`ValidatedJson`, `Json`) comes before a caller extractor (`AuthUser`, `AdminUser`, or a type the App lists as one) is a violation naming the handler, so a caller without permission gets 403 before the body's 422 reveals its rules. `rok add feature` (ticket 41) scaffolds handlers caller-first. The template docs say role checks belong in extractors, not services.

**Blocked by:** None for the rule; the template docs after grpc-api-template/08

**Status:** ready-for-agent

- [ ] A body-before-caller handler fails check-arch, naming it; caller-first passes
- [ ] The App can name its own caller extractors (config or marker)
- [ ] Template docs updated
