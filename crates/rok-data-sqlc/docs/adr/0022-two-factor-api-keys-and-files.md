# Two-factor sign-in, API keys, uploads and avatars in the templates

Date: 2026-10-26 (imported) · Status: accepted upstream, imported; in force here as the absorption tickets port it

Formerly rok-sqlc ADR 0020 (rok-sqlc 0.8.0). Imported under workspace ADR 0016. The text is upstream's. The terms are this workspace's: the `rok sqlc` front door, `rok.toml`, and engine ADR numbers after the import (upstream's ADR 0018 is engine ADR 0020 here, its 0019 is engine ADR 0021).

> **In this workspace.** Before the import, the workspace templates had the schema's `api_keys` and `media` tables and their generated data layer, but none of the hand-written features below. The absorption reconciles them to the behaviour below (the spec, `docs/specs/absorb-rok-sqlc-0.8.md`). Template migrations keep the workspace's versioned names (workspace ADR 0004). Where this ADR leans on engine ADR 0021's `AppState`/`FromRef` wiring, that is template code, which engine ADR 0021's superseded note leaves to the template.

Until rok-sqlc 0.8.0, both templates left eleven contract endpoints unimplemented: two-factor sign-in, API keys, uploads and profile photos. The login answer's `requires_totp` shape was declared but never used, and `avatar_url` was always missing. In 0.8.0 both templates implement them at once, as hand-written features in the generated features' layout. The generator learns enough about `ALTER TABLE … CONSTRAINT` for the template's own migration to widen an enum. The decisions are B1–B7 of `.scratch/rok-sqlc-0-8/notes/m6-decisions.md` (in the rok-sqlc repository), with the review fixes recorded below. Engine ADR 0020 (the account flows) and engine ADR 0021 (axum) still hold.

**Both templates, one copy of the rules (B1).**

- **What is shared:** each feature's `dto.rs` and `service.rs`, and everything in `shared/security/`, `shared/storage/` and `shared/thumbnail.rs`, is byte-identical in the actix and axum templates. The drift test lists them.
- **What is per framework:** handlers, routes, the extractors and `app.rs`.
- **The new features:** `two_factor` (`/auth/2fa/*`), `api_keys`, `media`, and the photo routes in `account`.
- **Why `two_factor` is its own feature:** it started as a block of `auth/service.rs`, and was split out when that file passed 600 lines. It has its own query module (`queries/two_factor.sql`). It has its own copies of `TokenResponse` and `MessageResponse` rather than reach into `auth`, as `account` keeps its own `UserResponse`.
- **Token lifecycle:** both features use the single-use token lifecycle in `shared/security/auth_token.rs`: issue, spend, and each purpose's lifetime and message (`policy`).

**Two-factor: TOTP, its secret sealed, its codes keyed (B2).**

- **Codes:** RFC 6238 (`totp-rs`: SHA-1, 6 digits, 30-second steps, one step of drift either way), compared in constant time (`subtle`).
- **The secret:** stored AES-256-GCM-sealed under `TOTP_ENCRYPTION_KEY` (32 bytes, base64, required and checked at startup like `JWT_SECRET`). The account's id is the associated data, so a sealed secret copied onto another row doesn't open.
- **No replay:** the last accepted time step is stored, and a conditional `UPDATE` spends it. So a code is never accepted twice, even by two requests at once.
- **Recovery codes:** ten single-use codes are shown once, at enable. They are stored as HMAC-SHA256 under a key derived from `TOTP_ENCRYPTION_KEY` (an HMAC of a fixed label), so a leaked table can't be checked against guesses without the key. Plain SHA-256 was the first design.
- **Zeroizing:** decrypted secrets and the derived key are zeroized when dropped (`zeroize`).
- **Setup:** answers the secret, its `otpauth://` URI and a QR code (an SVG data URI, so no image crate is needed for it). Nothing is enforced until enable confirms a code.
- **Sign-in with two-factor on:** the right password answers `{requires_totp, pending_token}`, where the token is a 5-minute single-use `two_factor_pending` token. Verify takes it with a code or a recovery code. A wrong code counts toward the same lockout as a wrong password (5 failures, 15 minutes).
- **Disable:** needs only the current password, because the contract's `DisableTotpDto` has only that field. To make up for it, disabling bumps `token_version`, signing every session out, the caller's included, as a password change does.
- **Notices:** enabling and disabling each mail a notice.

**Recovery revokes, a change keeps.**

- **A password reset:** it recovers an account someone else may have had. So its transaction revokes every active API key of the account and spends any pending two-factor sign-in, besides bumping `token_version`.
- **A password change:** it keeps the keys. Whoever changes the password knew the old one, and a key's owner revokes it explicitly.

**API keys (B4).**

- **The key:** `rok_<8>_<32>` (base62). Only its 12-character prefix (to recognise it in a list) and its SHA-256 (to look it up) are stored, and the raw key is answered once.
- **Sending it:** as `Authorization: Bearer rok_…` (told apart from a JWT by the prefix) or as `X-API-Key`. It works wherever a session does: `AuthUser` gains `credential` (`Session` or `ApiKey(scope)`).
- **Scopes:** a `read` key may only `GET`, `HEAD` and `OPTIONS`; anything else is a `403`.
- **Where keys can't go:** no key reaches `/api/v1/auth` or `/api/v1/api-keys` (`403`). Signing in, the account flows and managing keys need a session.
- **Path matching:** the path is compared after percent-decoding (until nothing decodes), resolving empty, `.` and `..` segments, and lower-casing. So it is never less decoded than what a router matches. A review found that actix's `AuthUser` checked the raw `req.path()` while actix-router routes the decoded path, so `/api/v1/api%2Dkeys` reached the keys with a key. actix now checks the matched path too.
- **Invalid keys:** expired or revoked keys, and keys of a disabled or deleted owner, are `401`.
- **`last_used_at`:** written at most once a minute.
- **Roles and limits:** a key acts with its owner's role, an admin's included, with no cap per key. A user has at most 25 active keys (`409`). Keys are created under a row lock on the owner, so concurrent creates can't pass the limit.

**The storage port and `object_store` (B5).**

- **The port:** features store bytes through `Storage` (`put`, `get`, `delete` by key, keeping the content type), in `shared/storage/`.
- **The implementation:** once, over `object_store` 0.13, with three backends:
  - `file://`, the development default (`./storage`);
  - `s3://bucket`, for S3 or any S3-compatible store, configured by the `AWS_*` variables;
  - `memory://`.
- **Why `object_store`:** it was chosen over the AWS SDK for its size, and because one crate covers local files, S3 and memory. 0.13 shares `reqwest` 0.12 with the breach check.
- **Keys:** server-generated, and `is_valid_key` keeps any key inside the store.
- **Whole bytes, not a stream:** `get` answers whole bytes. Every upload is capped, and bytes keep the port free of either framework's body type.
- **Opening the store:** `AppState::new` opens `STORAGE_URL`, so any application built from it stores where it is configured; the tests configure `memory://`. A review found that it silently started in memory, and only `run` swapped the real store in.

**Upload safety (B6).**

- **Size:** an upload is a raw body of at most `MEDIA_MAX_UPLOAD_BYTES` (10 MiB, `413`).
- **Type:** detected from the bytes (`infer`), never from `Content-Type` or the name. It must be one of `MEDIA_ALLOWED_TYPES` (JPEG, PNG, WebP, GIF, PDF), else `415 unsupported_media_type`, a new code of the wire standard.
- **Order of writes:** bytes are stored first, then the row, and the bytes are deleted if the row fails. Delete removes the row, then the bytes, best effort.
- **Serving:** content is served with the detected type, `X-Content-Type-Options: nosniff`, a `sandbox` CSP, and `Content-Disposition: attachment` for anything that isn't an image.
- **Ownership:** files are owner-scoped, so another user's is a `404`.
- **Reading the body after auth:** actix polls a handler's extractors together. So a `web::Bytes` parameter buffered the body while `AuthUser` was still checking the credential against the database, even for a caller then refused. Under actix, an upload handler therefore takes `web::Payload` and reads it after its extractors. axum runs extractors in order, so its body is read only after `AuthUser`.
- **The `413` body:** both frameworks' plain-text `413` becomes the wire standard's in `shared/body.rs`, with `AppError::too_large` building it.

**Avatars and the decoder's bomb limits (B7).**

- **The upload:** `PUT /users/me/avatar` takes at most 2 MiB.
- **Decoding:** JPEG, PNG or WebP, with `image` under `image::Limits` (8192×8192, 128 MiB of allocation). An undecodable or oversized image is a `422`.
- **Processing:** the centre is cropped to 256×256 and re-encoded as JPEG, which drops EXIF and any payload hidden in the original.
- **No GIF:** though the contract's description mentions it, one frame of an animation is not what a user picks.
- **Storing and serving:** the photo is stored as `avatars/<random>.jpg`, and the old one is deleted after the commit. It is served publicly at `/api/v1/avatars/<file>` with `Cache-Control: public, max-age=31536000, immutable`, since a new photo gets a new name.
- **`avatar_url`:** a generated column, `'/api/v1/' || avatar_key`. So the generated `users` feature and the hand-written features answer it with no generator change. The cost is that the `/api/v1/` prefix is written in the schema too.

**The generator follows `DROP` and `ADD CONSTRAINT … CHECK` (B3).**

- **Names:** checks and composite foreign keys now have names, declared or the ones PostgreSQL gives (`<table>_<col>_check`, `<table>_check`, `<table>_<cols>_fkey`, numbered when taken).
- **`DROP CONSTRAINT`:** drops a primary key, a unique constraint (and its lookup), a foreign key (and its relationship) or a check. Dropping a check recomputes its column's value list and ranges from the checks that remain.
- **`ADD CONSTRAINT … CHECK (col IN (…))`:** adds one. The template's `0003` migration widens `auth_tokens.purpose` this way, and `AuthTokenPurpose` gains `TwoFactorPending`.
- **`pg_dump`'s form:** an added check may also be written `col = ANY (ARRAY['a'::text, …])`. `CREATE TABLE` still reads only `IN (…)`, so code generated from existing schemas doesn't change.
- **Untracked names:** dropping a name the model doesn't track (an `EXCLUDE` constraint, PostgreSQL 18's `<table>_<col>_not_null`) first failed the whole table. It now warns, located at its statement, and changes nothing, since those constraints shape no generated code. `IF EXISTS` stays silent.
- **`RENAME CONSTRAINT`:** fails the table when the old name is unknown or the new one taken, as PostgreSQL does. It warns for a name in the form of an untracked kind (`…_not_null`, `…_excl`).

## Considered Options

- **One shared crate for the templates' common code.** Rejected again (engine ADR 0021): a template is copied and must build alone.
- **The AWS SDK, or one implementation per backend, for storage.** Rejected for `object_store`, one crate for local files, S3-compatible stores and memory.
- **Streaming `get`.** Rejected: uploads are capped, and bytes keep the port framework-free.
- **Plain SHA-256 recovery codes.** Rejected in review for a keyed HMAC: 50 bits of code are guessable offline from a leaked table.
- **Asking for a code to disable two-factor.** Not done: the contract's body has only `current_password`. Signing every session out on disable covers a stolen session instead.
- **A per-key role cap** (keys are never admin). Not done: a key is its owner acting from a script, and the owner revokes it.
- **OAuth, passkeys and QR login.** Not planned.

## Consequences

- `TOTP_ENCRYPTION_KEY` is required; a template project without it doesn't start.
- `AuthUser` accepts API keys and has a `credential` field. Code that matches it exhaustively or builds it by hand changes.
- Template migrations `0003_two_factor.sql`, `0004_api_keys.sql` and `0005_avatars.sql` are new; an existing project applies them in order.
- Both templates gain `aes-gcm`, `totp-rs`, `qrcode`, `object_store`, `infer`, `bytes`, `image`, `hmac`, `subtle` and `zeroize`.
- `AppState::new` returns a `Result` and opens `STORAGE_URL`. A test configures `memory://`, or swaps a store in with `with_storage`.
- `DROP CONSTRAINT`, which 0.7 refused as an unsupported clause (failing the table), is followed. A name the model doesn't track generates with a warning rather than failing the table.
