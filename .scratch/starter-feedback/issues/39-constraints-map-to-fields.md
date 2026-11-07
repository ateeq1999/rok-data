# 39: Constraints map to a field and a message

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:**
- **The annotation:** the engine reads `-- rok:constraint <name> [<field>] "<message>"` anywhere in the schema dir. It covers every constraint a user can break:
  - a `CHECK` or a trigger's `RAISE … USING CONSTRAINT = '<name>'` → 422 on the field
  - a unique constraint → 409
  - a foreign key → 409
  - the field is optional (a foreign key on delete has none)
- **The table:** it generates a constraint table (name → field, message) in the data layer.
- **The templates' error mapping:** a violation of a mapped constraint answers its status with the message and, when given, that field's detail. An unmapped one carries PostgreSQL's own message instead of "the value breaks the check …" or "the change breaks a reference (constraint …)".
- **Errors:** an annotation naming an unknown field or constraint is a generation error.

Karibu Pass's clients show `offer_drops_month_check` and `offer_drop_offers_offer_drop_id_fkey` as messages, and its web screens reword each 409 by hand (tickets 05, 06, 25).

**Blocked by:** None

**Status:** ready-for-agent

- [ ] The annotation parses for check, unique and foreign-key constraints; an unknown field or constraint fails, naming it
- [ ] The generated table lists every annotated constraint
- [ ] Template: a mapped CHECK is a 422 on its field with the message; a mapped foreign key is a 409 with its message; a trigger's RAISE text reaches the client
