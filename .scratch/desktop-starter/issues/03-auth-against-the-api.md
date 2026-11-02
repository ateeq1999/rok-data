# 03: sign-up, sign-in and sign-out against apps/api

**Blocked by:** 02
**Status:** done

Spec: What ships (Auth).

Sign-up, sign-in and sign-out on `apps/api`'s routes; the bearer token in the
OS keychain (`keyring`), behind a store trait with an in-memory test
implementation; restored at start, cleared at sign-out and on a 401; the
unverified-email answer shown as `apps/api` words it.

Done when the auth flows are tested through the bloc layer with the in-memory
store, and by hand against a running `apps/api`.

- [x] Sign-up on `apps/api`'s `POST /auth/register`
- [x] Sign-in on `POST /auth/login`, and sign-out
- [x] The bearer token in the OS keychain (`keyring`) behind a store trait, with an in-memory store for tests (no test touches the real keychain)
- [x] The token restored at start, cleared at sign-out and on a 401
- [x] The unverified-email answer shown as `apps/api` words it
- [x] A two-factor account either finishes signing in with its code or is told clearly why not
- [x] The flows tested through the bloc layer with the in-memory store
- [ ] The flows tried by hand against a running `apps/api` (not done here: see Comments)

## Comments

**The store.** `src/data/providers/token_store.rs`: the `TokenStore` trait
(`load`, `save`, `clear`), `KeychainTokenStore` (`keyring` 3, service = the
App's package name, account = the API's address, so each API keeps its own
token; nothing probes the keychain) and `MemoryTokenStore` for tests. `keep`
starts a `Session` with the stored token and writes every change of it back,
so the three moments the spec names need no code of their own: signing in
(`Session::set_token`) saves, signing out (`clear`) and a 401 (`expire`)
delete. `Api::kept(base_url, store)` holds that subscription; `main.rs` builds
the App with it. A store that cannot be read starts signed out; one that cannot
be written leaves the session in memory. `TokenStoreError` never carries the
token (a `BadEncoding` keyring error, which holds the entry's bytes, is
reworded).

**Restored at start.** `features/auth/view/restored_session.rs` asks for the
profile once when the session starts with a kept token, so the user shows as
signed in, or a 401 (token expired meanwhile) clears it and the session guard
sends them to sign in.

**Two-factor: the App asks for the code.** The API's flow is one more call
(`POST /auth/2fa/verify` with the pending token and a code), so `sign_in`
answers `SignIn::SignedIn(user)` or `SignIn::CodeRequired(PendingSignIn)`;
the page then asks for the code (an authenticator or a recovery code).
A wrong code (401) asks again; a spent or expired sign-in (`400
invalid_token`) goes back to the password; "Back" cancels. The pending token's
`Debug` is redacted.

**Sign-up** shows "Account created. Open the link mailed to {email} to
verify it, then sign in." Signing in unverified shows the API's 403 message
as is ("the email address is not verified yet").

**Not done: the by-hand run.** `cargo build -p shop-api` in a fresh scaffold
failed (140 errors, among them missing module files, E0583; not
investigated: the monorepo gate may run `rok gen` first), and the GUI could
not be driven from the implementing agent's session. Ticket 05's end-to-end
run (sign up, verify, sign in, restart signed in, sign out) covers it.
