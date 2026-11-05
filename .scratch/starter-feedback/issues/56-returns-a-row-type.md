# 56: `-- returns:` reuses a row type

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** `-- returns: FindCheckoutRow` makes a query return an existing row type of the same query file (or a model), instead of generating its own. If the columns differ in name, order, type or nullability, generation fails, naming each mismatch. Identical column lists are not merged without it.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Two queries share one row type; the generated code has one struct
- [ ] A mismatch fails naming the column and both types
