# 03: sign-up, sign-in and sign-out against apps/api

**Blocked by:** 02
**Status:** blocked

Spec: What ships (Auth).

Sign-up, sign-in and sign-out on `apps/api`'s routes; the bearer token in the
OS keychain (`keyring`), behind a store trait with an in-memory test
implementation; restored at start, cleared at sign-out and on a 401; the
unverified-email answer shown as `apps/api` words it.

Done when the auth flows are tested through the bloc layer with the in-memory
store, and by hand against a running `apps/api`.
