# 08: Storage port, uploads and avatars (rok-sqlc 0.8)

**What to build:** Uploads go through a `Storage` port opened from `STORAGE_URL` (local disk or S3-compatible). `/api/v1/media` accepts a size-capped raw body (`413` too large, `415 unsupported_media_type` disallowed type); profile photos can be set, removed and served publicly. Actix reads an upload's body only after `AuthUser` succeeds. The partial storage/avatar pieces already in this workspace are reconciled to upstream 0.8 behavior.

**Blocked by:** 07

**Status:** ready-for-agent

- [ ] Both templates; new dependencies only in the templates' own manifests
- [ ] Template tests: media, avatars (DB-backed, printed skip)
- [ ] CHANGELOG: `STORAGE_URL`, the 415 answer
