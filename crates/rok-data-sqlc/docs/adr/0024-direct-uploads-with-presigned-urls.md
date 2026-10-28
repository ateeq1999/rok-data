# Direct uploads and downloads through presigned URLs

Date: 2026-10-28 · Status: accepted

Engine ADR 0022 gave both templates a `media` feature whose bytes pass through the API: an upload is a raw body capped at `MEDIA_MAX_UPLOAD_BYTES` (10 MiB), and content is served by the API. That caps file size by what the API will buffer, and it spends the API's bandwidth on every byte. S3 and the stores compatible with it (MinIO, R2, …) can take and serve bytes directly, from a **Presigned URL**. This ADR adds that path beside the proxied one.

**The upload flow.**

1. **The intent:** `POST /api/v1/media/uploads` with the declared `size_bytes`, `content_type` and `original_filename` creates an **Upload intent**. It answers `{media_id, upload_url, expires_at}`: a presigned `PUT` whose signature covers `Content-Type`, valid for `MEDIA_UPLOAD_URL_TTL_SECS` (900 by default).
2. **The bytes:** the client `PUT`s them straight to the store.
3. **Completion:** `POST /api/v1/media/{id}/complete` reads the object's head. A size or content type other than declared, or a size over `MEDIA_MAX_UPLOAD_BYTES`, deletes the object and the intent and answers `422`. Otherwise the file is ready.

**The intent is a `media` row.**

- **Its columns:** an upload intent is a `media` row with `completed_at` null and an `expires_at`. The id the client holds is the same from the intent to the finished file.
- **Scoping:** every owner-scoped `media` query reads only completed rows (`completed_at IS NOT NULL`). Generated reads carry the condition, so no caller has to remember it.
- **The sweep:** a periodic task in the application, every `MEDIA_INTENT_SWEEP_SECS`, deletes expired intents and any object they left.

**Downloads.**

- **The redirect:** `GET /api/v1/media/{id}/content` answers `302` to a presigned `GET`, valid for `MEDIA_DOWNLOAD_URL_TTL_SECS` (300 by default).
- **Clients that can't follow it:** with `?redirect=false`, it answers `{url, expires_at}` as JSON instead. This is for clients that can't follow a redirect while sending an auth header.
- **The filename:** the original filename is kept on the row and answered in the metadata. It is not forced into `Content-Disposition`: `object_store`'s signer offers no response-header overrides.

**Stores that can't presign.**

- **API-signed URLs:** `file://` and `memory://` answer URLs that the API signs itself (an HMAC over method, key and expiry), served at `PUT`/`GET /api/v1/storage/{token}`.
- **Why:** a client's flow is the same in development, in tests and in production, and the tests need no MinIO.

**S3-compatible stores.**

- **Configuration:** they are configured by `AmazonS3Builder::from_env` (`AWS_ENDPOINT`, `AWS_ALLOW_HTTP`, `AWS_VIRTUAL_HOSTED_STYLE_REQUEST`), documented in `.env.example`.
- **`STORAGE_PUBLIC_ENDPOINT`:** it rewrites the host of a presigned URL. A URL signed for `http://minio:9000` inside Docker is unreachable from a browser otherwise.

**The proxied upload stays.** `POST /api/v1/media` (raw body) and `PUT /users/me/avatar` keep engine ADR 0022's behaviour: avatars are decoded and re-encoded by the API, which a direct upload cannot do.

**Both templates, one copy.** The `Storage` port gains presigning. As engine ADR 0022 requires for `shared/storage/`, it is byte-identical in the actix and axum templates, and only the handlers and routes differ.

## Considered Options

- **A separate `media_uploads` table, its row moved into `media` on completion.** Rejected: two rows and two ids for one file, and a move that can half-fail.
- **Refusing presigned flows unless S3 is configured.** Rejected: development and tests would need a different client flow, or MinIO.
- **Telling the client `mode: direct | proxy`.** Rejected: every client would carry both flows.
- **S3 POST policies, which enforce a byte range at upload.** Not available through `object_store`. Checking on completion catches the same uploads after the fact, and the sweep removes what is never completed.
- **Direct upload only, removing the proxied path.** Rejected: avatars need the API to process the bytes.

## Consequences

- A new template migration adds `completed_at` and `expires_at` to `media`. Existing rows are backfilled as completed.
- `shared/storage/` is a scaffolded file: an existing project keeps its copy and adopts the change by hand. New projects get it.
- An object's bytes may exist briefly without a ready row (between `PUT` and completion, or until the sweep). Nothing reads them, since every read is through a completed row.
