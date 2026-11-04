# 31: Casts keep nullability

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in the engine's query result typing, a cast keeps the nullability of the expression it wraps. `count(*)::bigint` is `i64`, a `NOT NULL` column cast to another type is not-null, and `coalesce(x, <literal>)` is not-null; every other expression stays nullable. The `-- column: <name> <type>` override is documented for the cases inference can't settle.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `count(*)::bigint AS owners` in a `:one` query generates `i64` (red first)
- [ ] A nullable column cast stays `Option<_>`
- [ ] `-- column:` override documented in the queries guide
