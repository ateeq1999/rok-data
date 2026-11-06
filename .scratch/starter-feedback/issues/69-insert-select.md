# 69: `INSERT … SELECT`

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** typed queries support `INSERT INTO t (cols) SELECT …`, with the inserted columns typed from the target table as `VALUES` already is, parameters typed from the column they fill (or from the `SELECT`'s own expressions), and `ON CONFLICT` and `RETURNING` as for `VALUES`. Today it fails "INSERT … SELECT is not supported yet; use VALUES".

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Karibu's `AddPhoto` shape (`INSERT … SELECT $1, $2, $3, $4, coalesce(max(sort) + 1, 0) FROM partner_photos WHERE partner_id = $2 ON CONFLICT …`) generates with `:exec` and with `RETURNING`
- [ ] A column-count mismatch fails naming it
