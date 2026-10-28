# Direct uploads and downloads through presigned URLs

Date: 2026-10-28 · Status: accepted

Engine ADR 0022 gave both templates a `media` feature whose bytes pass through the API: an upload is a raw body capped at `MEDIA_MAX_UPLOAD_BYTES` (10 MiB), and content is served by the API. That caps file size by what the API will buffer, and it spends the API's bandwidth on every byte. S3 and the stores compatible with it (MinIO, R2, …) can take and serve bytes directly, from a **Presigned URL**. This ADR adds that path beside the proxied one.

**The upload flow.**

1. **The intent:** `POST /api/v1/media/uploads` with the declared `content_type`, `size_bytes` and optional `filename` checks the size (`413`) and type (`415`) up front and creates an **Upload intent**. It answers an upload ticket, `{media_id, method, upload_url, headers, expires_at}`: a presigned `PUT` whose signature covers `Content-Type`, the header the ticket names, valid for `MEDIA_UPLOAD_URL_TTL_SECS` (900 by default).
2. **The bytes:** the client `PUT`s them straight to the store.
3. **Completion:** `POST /api/v1/media/{id}/complete` reads the object's size and first 8 KiB. A size other than declared, or a type detected from the bytes other than declared, deletes the object and the intent and answers `422`; no object yet is a `409`. Otherwise the file is ready. The type is detected from the bytes, as for a proxied upload, not taken from the store's metadata, which the client chose.

**The intent is a `media` row.**

- **Its columns:** an upload intent is a `media` row with `completed_at` null and an `expires_at`. The id the client holds is the same from the intent to the finished file.
- **Scoping:** every owner-scoped `media` query reads only completed rows (`completed_at IS NOT NULL`). Generated reads carry the condition, so no caller has to remember it.
- **The sweep:** a periodic task in the application, every `MEDIA_UPLOAD_SWEEP_SECS` (300 by default; `0` turns it off), deletes expired intents and any object they left.

**Downloads.**

- **A new endpoint:** `GET /api/v1/media/{id}/url` answers `{url, expires_at}`, a presigned `GET` valid for `MEDIA_DOWNLOAD_URL_TTL_SECS` (300 by default).
- **`/content` is unchanged:** it still answers the bytes. The contract spec (`tests/spec/openapi.json`) pins it as a `200`, and a redirect breaks clients that can't follow one while sending an auth header.
- **The filename:** the original filename is kept on the row and answered in the metadata. It is not forced into `Content-Disposition`: `object_store`'s signer offers no response-header overrides.

**Stores that can't presign.**

- **API-signed URLs:** `file://` and `memory://` answer URLs that the API signs itself (an HMAC-SHA256 over method, key and expiry, under a key derived from `JWT_SECRET`, so no new secret has to be configured and rotating `JWT_SECRET` revokes every outstanding URL), served at `PUT`/`GET /api/v1/storage/{token}`.
- **Why:** a client's flow is the same in development, in tests and in production, and the tests need no MinIO.

**S3-compatible stores.**

- **Configuration:** they are configured by `AmazonS3Builder::from_env` (`AWS_ENDPOINT`, `AWS_ALLOW_HTTP`, `AWS_VIRTUAL_HOSTED_STYLE_REQUEST`), documented in `.env.example`.
- **`STORAGE_PUBLIC_ENDPOINT`:** presigned URLs are signed by a second client built for that endpoint. A URL signed for `http://minio:9000` inside Docker is unreachable from a browser otherwise, and rewriting its host after signing would break the SigV4 signature, which covers the host.

**The proxied upload stays.** `POST /api/v1/media` (raw body) and `PUT /users/me/avatar` keep engine ADR 0022's behaviour: avatars are decoded and re-encoded by the API, which a direct upload cannot do.

**Both templates, one copy.** The `Storage` port gains presigning. As engine ADR 0022 requires for `shared/storage/`, it is byte-identical in the actix and axum templates, and only the handlers and routes differ.

## Considered Options

- **A separate `media_uploads` table, its row moved into `media` on completion.** Rejected: two rows and two ids for one file, and a move that can half-fail.
- **Refusing presigned flows unless S3 is configured.** Rejected: development and tests would need a different client flow, or MinIO.
- **Telling the client `mode: direct | proxy`.** Rejected: every client would carry both flows.
- **S3 POST policies, which enforce a byte range at upload.** Not available through `object_store`. Checking on completion catches the same uploads after the fact, and the sweep removes what is never completed.
- **Direct upload only, removing the proxied path.** Rejected: avatars need the API to process the bytes.
- **A `302` from `/content` to the presigned URL, `?redirect=false` for JSON.** Rejected: it changes an operation the contract spec pins.
- **A separate `STORAGE_SIGNING_KEY`.** Rejected: one more secret to configure for every deployment, with nothing gained over a key derived from `JWT_SECRET` under its own label.

## Consequences

- A new template migration adds `completed_at` and `expires_at` to `media`. Existing rows are backfilled as completed.
- `shared/storage/` is a scaffolded file: an existing project keeps its copy and adopts the change by hand. New projects get it.
- An object's bytes may exist briefly without a ready row (between `PUT` and completion, or until the sweep). Nothing reads them, since every read is through a completed row.
