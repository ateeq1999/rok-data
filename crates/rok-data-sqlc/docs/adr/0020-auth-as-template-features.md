# Auth as hand-written template features: sessions, single-use tokens, a mail outbox

Date: 2026-10-26 (imported) · Status: accepted upstream, imported; in force here as the absorption tickets port it

Formerly rok-sqlc ADR 0018 (rok-sqlc 0.6.0). Imported under workspace ADR 0016. The text is upstream's. The terms are this workspace's: the `rok sqlc` front door, `rok.toml`, and engine ADR numbers after the import.

> **In this workspace.** The templates are `crates/rok-data-sqlc/templates/actix-api/` and `axum-api/`. Until the absorption's template tickets land, they hold only the generated features. Template migrations keep the workspace's versioned names (workspace ADR 0004), which `0002_auth.sql` already is. The `cross-feature-query` rule below joins this workspace's `rok sqlc check-arch`.

Until rok-sqlc 0.6.0, `rok sqlc init --template actix` made an API that checked a JWT something else had to issue. Nobody could sign up, sign in, verify an email, or reset or change a password. The target contract (the axum starter's `openapi.json`) defines those flows, and generated code can't express them: they need hashing, single-use tokens, mail delivery and rate limits. So in 0.6.0 (milestone 5) the template gains them as hand-written features. The decisions are Q1–Q26 of `.scratch/rok-sqlc-0-6/notes/m5-auth-decisions.md` (in the rok-sqlc repository). The user's guide is `docs/guide/auth.md`.

**Auth is template code, not generated code.**

- **Where the flows live:** in `templates/actix-api/` as `features/auth/` and `features/account/`, in the generated features' folder layout (`mod`, `routes`, `handlers`, `dto`, `service`). They are user-owned, and `check-arch` checks them in full.
- **Shared pieces:** mail goes in `shared/mail/`, hashing and tokens in `shared/security/`, and the rate limiter in `shared/rate_limit.rs`.
- **What the generator does:** it emits no auth. A plain `rok sqlc init` keeps only the `AuthUser`/`AdminUser` check.
- **`[api.verifiable]` is dropped:** the idea (Q55 of 0.4) was to generate a verified-change flow for a column. But email change and password change are two flows with different rules: a password is never mailed, and an email change needs the new address to prove itself. A generator option for exactly those two would be a template in disguise. `[api.immutable]` still keeps `email` out of the generated `PATCH /users/{id}`.

The generator gains one thing: the `check-arch` rule `cross-feature-query`.

- A feature calls its own query module, `<db_path>::queries::<feature>` (from `queries/<feature>.sql`), whatever tables those queries touch. It never calls another feature's.
- A query module named after no feature is anyone's. Examples: the outbox's `queries/mail.sql`, and `queries/session.sql`, which holds the session check `shared/auth.rs` runs, so `shared/` never reaches into the `auth` feature's queries.
- Writing another feature's data still goes through that feature's service behind a port (engine ADR 0016). For example, invitations record their audit entry through an `AuditTrail` port onto the `audit_log` service's `create_in`, on the invitation's transaction.

**The session is an access-only JWT, checked against the database on every request.**

- **The token:** sign-in answers the contract's `TokenResponse`, an HS256 JWT with `sub`, `role`, `tv` and `exp`, valid for `JWT_TTL` (1 hour). There is no refresh token, as the contract has none.
- **The per-request check:** `AuthUser` decodes the token, then loads the account's `token_version`, `is_active` and `role` in one query. A deleted or disabled account is a `401`, as is a token whose `tv` isn't the account's `token_version`.
- **Roles:** the role comes from the database, so `AdminUser` sees a promotion or demotion on the next request.
- **Revocation:** a password change or a reset bumps `token_version`, signing every older session out. Disabling an account bumps nothing: the same per-request `is_active` check rejects its tokens (Q21).
- **Cost:** one indexed query per authenticated request, which is why `AuthUser` needs the pool (`web::Data<PgPool>`).
- **The secret:** `JWT_SECRET` must be at least 32 bytes, the size of the HMAC-SHA-256 output. A shorter one stops the server at start-up.

**Passwords are argon2id, and the policy is length only.**

- Hashing and verifying run on actix's blocking pool.
- A password is 8 to 128 characters, with no composition rules (NIST 800-63B).
- Sign-in with an unknown email verifies against a dummy hash, so the timing doesn't say which emails exist.
- An optional breach check (`BREACH_CHECK=true`) asks Have I Been Pwned's range API with 5 hex characters of the SHA-1 (k-anonymity). It fails open with a warning, and applies to register, reset and change.
- Five wrong passwords in a row lock an account for 15 minutes. A reset lifts the lock.

**Mailed links are single-use, hashed tokens.**

- **One table:** `auth_tokens` holds every link. Its `purpose` is one of:
  - `email_verification` (24 h);
  - `password_reset` (1 h);
  - `email_change` (1 h, with `new_email`);
  - `invitation` (7 days).
- **Storage:** a token is 32 random bytes, and only its SHA-256 is stored, so a leaked table holds no usable link.
- **Issuing and redeeming:** issuing one spends the user's older unused tokens of that purpose. Redeeming one is a single `UPDATE … RETURNING` (unused, unexpired, right purpose), so two requests can't both spend it.
- **Invitations:** an invitation is redeemed through the password reset, which also verifies the email.

A reset runs in this order:

1. It looks the token up read-only. An unknown, expired, used or wrong-purpose token is a `400 invalid_token`, and costs no breach request and no hash.
2. The breach check runs.
3. The password is hashed, outside the transaction.
4. One transaction spends the token and writes the password.

So a `400 breached_password` or a `422` leaves the link usable, and the transaction holds its connection for only two statements.

**Mail goes through an outbox, sent at least once.**

- **Writing:** a flow writes its message as an `outbound_mail` row on its own transaction, so a rolled-back flow sends nothing. It commits through the outbox, which wakes the sender.
- **Sending:** one tokio task per process (started in `app.rs`, stopped on shutdown) claims up to 10 due rows with `FOR UPDATE SKIP LOCKED`. It sends them through the `Mailer` port (SMTP with lettre and rustls, or the log) and deletes each one sent.
- **Failures:** a failed send backs off (30 s, doubling) until the fifth failure makes the row `dead`.
- **At least once:** the sender holds the batch's transaction while sending. That keeps a second instance off the same rows, and it means a crash after a send but before the commit sends that mail again. At-least-once is the right trade for account mail, which is idempotent for the reader (a second identical link).
- **No queue:** there is no general jobs queue, and `/health/ready` doesn't depend on mail.

A pending row has to carry its link, since it is what gets sent. A sent row is deleted. A `dead` row loses its text and HTML bodies but keeps its address, subject and `last_error`, to say what never left. So no raw token outlives its delivery.

**Flow failures are `400`s with their own codes; shape failures stay `422`.**

- **Shape failures:** a body of the wrong shape (an email that isn't one, a password outside 8–128 characters) answers the wire standard's `422 validation_failed`, with field details.
- **Flow failures:** a well-formed request that a flow refuses answers `400` with a specific code:
  - `invalid_token`: a bad, expired or used link;
  - `wrong_password`: the current password a flow asks for again;
  - `breached_password`.

  The contract's documents list `400` for these; ours list both.
- **Sign-in:** bad credentials and a locked account both answer `401`, with one message for all. That is decided before `403`, which needs the right password. A `403` means a disabled account, or an unverified email while `REQUIRE_VERIFIED_EMAIL` is on.

**What answers the same either way, and what doesn't.**

- `forgot`, `resend` and `email/change` (with the right password) always answer `202`. An email change mails a notice to the old address whether or not the new one is free, and a link to the new address only when it is. A taken address is found out only at confirm (`400 invalid_token`), by whoever holds both mailboxes.
- `register` answers `409` for a taken email, as the contract does. It tells a caller which emails have accounts; the `/auth` rate limit bounds how fast.
- `forgot` for a known email does more work (a token and a mail row) than for an unknown one. The answer is the same `202`. The timing difference is small and bounded by the rate limit, and not worth a fake write.
- Change-password and email-change check the current password without the sign-in lockout. Locking there would let a stolen session lock the owner out; the `/auth` rate limit bounds guessing instead.

**One rate limit on `/auth`, in process.**

- **Scope:** every `/auth` route (and `GET /auth/me`) shares a keyed GCRA limiter per client IP: `AUTH_RATE_LIMIT` requests a minute (20; `0` turns it off).
- **Answer:** `429 too_many_requests` with `Retry-After`.
- **Implementation:** a small middleware over `governor` (MIT). `actix-governor`, which Q9 named, is GPL-3.0-or-later, which a template that users ship can't carry.
- **The client's IP:** the peer's, or with `TRUST_PROXY=true` the last `X-Forwarded-For` address, the one the proxy in front appended. That is right behind exactly one proxy. Behind a CDN and a load balancer, the last hop is the CDN's address. So configure the last proxy to put the client's IP last, or turn the limiter off and limit at the edge.
- **Per process:** several instances each allow the limit.

**Deferred and not planned.**

- `LoginResponse` keeps the contract's `TotpRequired { requires_totp, pending_token }` variant for compatibility. Sign-in never answers it until two-factor lands.
- Milestone 6 is built once for both templates, after the axum template (0.7): two-factor (TOTP and login's `pending_token`), API keys (with an `ApiKey` extractor), and media and avatars (with a storage port).
- Not planned: OAuth, passkeys and QR sign-in.

## Considered Options

- **Generate auth from configuration (`[api.verifiable]`, an `[auth]` section).** Rejected: the flows are rules, not shapes, and every application bends them. A template the user owns can be changed; generated code can't.
- **A refresh token, or server-side sessions.** Rejected for now: the contract's `TokenResponse` has neither, and the per-request account check already gives revocation (`token_version`, `is_active`) without a session table.
- **Trust the token's role and `tv` until it expires.** Rejected: a disabled account or a changed password would keep working for up to `JWT_TTL`.
- **Send mail inside the request.** Rejected: an SMTP outage would fail sign-ups, and a mail would leave for a transaction that then rolls back.
- **A general jobs queue for the outbox.** Rejected: one table and one task cover account mail, and a queue is a bigger decision than auth.
- **Answer every email-change request with no mail at all when the address is taken.** Rejected (it was 0.6's first cut): the old address would get a notice for a free address and none for a taken one, which tells the caller who has an account.
- **Lock out on wrong current passwords too.** Rejected: as above, a stolen session could lock the owner out.

## Consequences

- `AuthUser` makes a database query per request and needs `web::Data<PgPool>` registered, which the template's `app::configure` does.
- The template has a second migration, `0002_auth.sql` (`users.token_version`, `auth_tokens`, `outbound_mail`).
- `AppState` holds the outbox, templates, breach check, rate limit and audit port, so tests build it with `AppState::new`.
- Projects made from an older template can copy the features; nothing generated changes for them.
- Moving to several instances needs nothing for the outbox (`SKIP LOCKED`), but needs a shared limiter, or a limit at the edge, for the rate limit.
