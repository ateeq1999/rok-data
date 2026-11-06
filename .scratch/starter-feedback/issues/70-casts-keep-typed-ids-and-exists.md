# 70: Casts keep a typed id, and `EXISTS`

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in query result typing:
- **Typed ids:** a cast to a column's own base type keeps the column's typed id. `coalesce(p.cover_media_id, (SELECT ph.media_id …))::uuid`, both sides `media.id`, is `Option<MediaId>`, not `Option<uuid::Uuid>`.
- **`EXISTS (…)`:** a `bool` that is never null (ticket 51 missed it), and so is `NOT EXISTS`.
- **`-- column:`:** it accepts the generated typed ids and enums (`-- column: cover_media_id Option<MediaId>`, `types::PartnerKind`) as `-- param:` does. Verify, and fix if it doesn't.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] The `coalesce(...)::uuid` over two `media.id` columns is `Option<MediaId>`
- [x] `(EXISTS (…))::boolean` is `bool`
- [x] `-- column:` with a typed id and an enum generates those types
