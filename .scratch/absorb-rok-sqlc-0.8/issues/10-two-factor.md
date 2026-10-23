# 10: Two-factor sign-in (rok-sqlc 0.8)

**What to build:** A separate template feature for two-factor sign-in: setup, enable, disable, login challenge, recovery codes. Secrets are sealed with `TOTP_ENCRYPTION_KEY` and bound to their account; recovery codes are keyed HMACs; disabling two-factor signs every session out; a password reset revokes pending two-factor sign-ins.

**Blocked by:** 09

**Status:** ready-for-agent

- [ ] Feature can be left out at `init`; both templates
- [ ] Versioned two-factor migration in the workspace's format
- [ ] Template `two_factor` tests ported
- [ ] CHANGELOG: `TOTP_ENCRYPTION_KEY`, migration, new dependencies
