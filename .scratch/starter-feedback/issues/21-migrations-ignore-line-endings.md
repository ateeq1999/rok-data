# 21: Line endings don't change migrations

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:**
- **Staging:** migration staging (rok-data-codegen: truth files, the `.rok-schema.json` snapshot, the diff) normalises CRLF to LF before comparing, as the engine already does when it reads schema and query files.
- **Checksum:** rok-data-core's migration checksum is computed over LF-normalised text, so a CRLF checkout of a `.up.sql` keeps its checksum.
- **Migration guide:** notes the one-time checksum change for a migration recorded from a CRLF file, and how to re-record it.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] A CRLF schema file whose statements equal the snapshot's stages nothing (the Karibu case: `touch_updated_at()` and its trigger)
- [x] A CRLF and an LF copy of one migration have the same checksum
- [x] Changelog and migration guide updated
