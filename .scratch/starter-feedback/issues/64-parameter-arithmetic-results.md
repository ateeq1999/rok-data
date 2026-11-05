# 64: Results of parameter arithmetic

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in query result typing, an arithmetic operand that is a placeholder takes the type parameter typing gave it (ticket 54 types `$1` in `price * $1` from the numeric column), so the result column `price * $1 AS x` is typed like `price * price`, nullable when the column is. Result typing runs before parameter typing today, so this needs the placeholder's type available to result typing (or a second pass).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `price * $1 AS x` on a `NOT NULL numeric` column is `Decimal`; on a nullable one `Option<Decimal>`
- [ ] A placeholder with no numeric partner still fails with the hint
