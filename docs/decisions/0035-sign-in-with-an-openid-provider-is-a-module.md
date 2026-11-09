# ADR 0035: Sign-in with an OpenID provider is a Module that links by email only for trusted providers

Date: 2026-11-09 · Status: accepted (amends ADR 0033)

## Context

Karibu Pass's ticket 26 hand-wrote "sign in with Google" on the
Monorepo template: three tables, a provider seam with PKCE, a
single-use state and a one-minute hand-over code, link and unlink
routes, and a fake provider for tests, about 600 lines. Every App that
wants sign-in with a provider writes the same thing, and the 0.5.0 spec
had planned only a recipe page for it.

## Decision

- **`oidc` is a Module** (ADR 0033) of the API Templates and the
  Monorepo template, kept by default and left out with
  `--without oidc`. Like a Capability, it is inert until configured:
  with no `OIDC_*` provider set, its routes answer 404 and its buttons
  are hidden (ADR 0023's switch). It needs only the core auth; when
  `totp` is kept, a User with two-factor on still gives the code.
- **Generic OpenID Connect, providers keyed by name**
  (`/auth/oidc/{provider}/start`), each configured by issuer, client id
  and secret. Google is the documented first provider; nothing is
  Google-only.
- **A Linked identity is matched by provider and subject, never by
  email**, unless the provider is configured `trust_email = true`.
  Without it, an unknown identity whose email belongs to a User is
  refused with a message to sign in and link it from settings; an
  unknown identity with a new email creates a User. With it, a verified
  email links to the User who has it. The Google setup sets it.
- **Unlinking keeps a way in:** removing the last Linked identity of a
  User with no password is refused with a 422 that says why.

## Considered Options

- **`rok init --with oidc`:** opt-in matches how rarely some projects
  need it, but it is a second flag beside `--without`, and every other
  auth surface ships kept by default.
- **A recipe page:** no Template code to maintain, but it leaves each
  App to rewrite the same 600 lines and the same security choices.
- **Always link by a verified email:** the smoothest sign-in, but it
  trusts every provider's `email_verified`, and linking on a provider
  that verifies emails loosely hands that provider's users other
  people's accounts.

## Consequences

- `trust_email` is a per-provider security setting, so the guide says
  what it trusts and when not to set it.
- The start and callback routes answer only redirects, so the contract
  and the Clients carry Browser routes (starter-feedback ticket 85)
  before the Module can ship.
