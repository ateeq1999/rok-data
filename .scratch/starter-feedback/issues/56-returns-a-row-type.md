# 56: `-- returns:` reuses a row type

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** `-- returns: FindCheckoutRow` makes a query return an existing row type of the same query file (or a model), instead of generating its own. If the columns differ in name, order, type or nullability, generation fails, naming each mismatch. Identical column lists are not merged without it.

`-- returns:` may also name a hand-written type by path (`-- returns: crate::features::payouts::dto::PayoutSummary`). The query then decodes straight into it (the type must derive `sqlx::FromRow`), no row struct is generated, and nothing is copied field by field. The engine can't see a hand-written type's fields, so a mismatch is caught by the compiler and by the row decode, not by `rok gen`; the docs say so. No `From<XRow>` impls are generated.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Two queries share one row type; the generated code has one struct
- [ ] A mismatch fails naming the column and both types
- [ ] A query returning a hand-written `FromRow` DTO generates no row struct and decodes into it (DB-backed test)
