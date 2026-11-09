# 88: The `oidc` Module: sign-in with an OpenID provider

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it) · workspace ADR 0035

**What to build:** a Module `oidc` in the actix, axum and Monorepo API Templates, kept by default, left out with `--without oidc`, and inert until a provider is configured.

- **Data:** `user_identities` (a User's **Linked identities**, unique by provider and subject), `oidc_requests` (single-use state, PKCE verifier, nonce, expiry) and `oidc_handoffs` (ticket 89).
- **Providers:** generic OpenID Connect, keyed by name, each configured by issuer, client id, secret and `trust_email`; Google is the documented example and sets `trust_email = true`.
- **Seam:** `IdentityProvider`, with an `openidconnect` implementation and a test fake.
- **Routes:** `GET /auth/oidc/{provider}/start` and `/callback` (Browser routes), and list, link and unlink of the signed-in User's identities.
- **Rules:** an unknown identity whose email belongs to a User is refused unless the provider is trusted; a new email creates a User; a kept `totp` asks for the code; unlinking the last way in (no password, no other identity) is a 422 saying why.
- **Guide:** `docs/guide/auth.md` gains "Sign in with Google", including what `trust_email` trusts.

Karibu Pass hand-wrote this in about 600 lines (ticket 26).

**Blocked by:** 85

**Status:** blocked

- [ ] Sign-in, sign-up and linking against the fake provider (API tests), including the refused email match and the 422 on the last unlink
- [ ] A kept `totp` asks for the code after the provider
- [ ] `--without oidc` builds and passes (the all-out gate); unconfigured, the routes answer 404
