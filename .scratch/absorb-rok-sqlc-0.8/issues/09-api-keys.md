# 09: API keys (rok-sqlc 0.8)

**What to build:** Users list, create and revoke API keys; `AuthUser` accepts a key as a bearer token; an encoded path can never take a key to `/auth` or `/api-keys`; a password reset revokes the account's keys.

**Blocked by:** 07

**Status:** done

- [x] Both templates; template `api_keys` tests ported
- [x] Encoded-path guard test
- [x] Reset-revokes-keys test
