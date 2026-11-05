# 51: Everyday expressions are typed

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** query result typing understands:
- **Comparisons and boolean logic** (`=`, `<`, `<=`, `BETWEEN`, `IN`, `IS NULL`, `AND`, `OR`, `NOT`): `bool`.
- **Arithmetic:** the operands' shared type, with `numeric` winning over integers.
- **`CASE`:** the type its branches share.
- **`||`:** text.

Each is nullable when an operand is (`IS NULL` never is), so `expires_at <= now()` on a `NOT NULL` column is `bool`. Anything else still fails under ticket 20 with the `-- column:` hint.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `expires_at <= now() AS expired` is `bool`; on a nullable column `Option<bool>`
- [x] `price * 0.8`, `CASE … END`, `a || b` typed as above (tests per form)
- [x] Mixed or unknown operand types still fail with the hint
