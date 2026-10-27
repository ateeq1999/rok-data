# 10: Two-factor sign-in (rok-sqlc 0.8)

**What to build:** A separate template feature for two-factor sign-in: setup, enable, disable, login challenge, recovery codes. Secrets are sealed with `TOTP_ENCRYPTION_KEY` and bound to their account; recovery codes are keyed HMACs; disabling two-factor signs every session out; a password reset revokes pending two-factor sign-ins.

**Blocked by:** 09

**Status:** done, except leaving it out at `init` (see the note)

- [ ] Feature can be left out at `init`; both templates
- [x] Versioned two-factor migration in the workspace's format
- [x] Template `two_factor` tests ported
- [x] CHANGELOG: `TOTP_ENCRYPTION_KEY`, migration, new dependencies

**Note (open):** `two_factor` is its own feature (directory, queries,
routes), but `init` has no option to leave it out: `auth`'s sign-in
answers `requires_totp`, and `users.totp_*`, `0003_two_factor`, config
(`TOTP_ENCRYPTION_KEY`), `openapi.rs` and the mail templates reference
it. Leaving it out at `init` needs a template variant (or an
`--without two-factor` that strips those pieces) with its own gate;
tracked as a follow-up, not done here.
