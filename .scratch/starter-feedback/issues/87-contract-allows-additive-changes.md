# 87: The contract test allows additive changes

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in the actix, axum and Monorepo API Templates' `tests/contract.rs`, a covered operation may add to the target without a `KNOWN` entry: new optional request-body properties, new optional query parameters, new properties in a response body. A removed or retyped property, a new required property or parameter, and a changed success status still fail. The module doc says what counts as additive; ticket 28's `KNOWN`/`KNOWN_SCHEMAS` stays for real departures.

Karibu Pass added the Customer's language, home city and texts to `PATCH /users/me`, the contract failed, and the settings went to `PATCH /users/me/settings` beside it: two routes updating one row (ticket 26).

**Blocked by:** None (can start immediately)

**Status:** done (rok-templates PR #16)

- [x] Adding an optional property to `PATCH /users/me`'s body passes the contract (test)
- [x] Adding a required property, or removing one, still fails, naming the operation
- [x] The same in actix, axum and the Monorepo API
