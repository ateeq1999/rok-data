# 89: OIDC on mobile and web: the hand-over and the buttons

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it) · workspace ADR 0035

**What to build:** in the Monorepo template, `start?client=mobile` makes the callback redirect to `<APP_SCHEME>://auth/oidc?code=…` with a one-minute, single-use hand-over code, and `POST /auth/oidc/handoff` exchanges it for a session. The Flutter App opens the start URL (ticket 85's builder) in the system browser, handles the deep link and signs in. The web App shows "Continue with <provider>" for each configured provider. Both hide the buttons when none is configured, and their settings screens list, link and unlink identities.

Karibu Pass's recipe covered the browser only (ticket 26).

**Blocked by:** 88

**Status:** done (rok-templates PR #18, b8ec9fe; the Apps build the start URL by hand until a rok release adds `oidcStartUrl`)

- [x] The hand-over code works once and expires after a minute (API test)
- [x] The Flutter App signs in from the deep link (widget test with a fake link)
- [x] The web App's buttons follow the configured providers (component test)
