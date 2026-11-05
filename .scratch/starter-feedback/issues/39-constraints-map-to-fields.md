# 39: Constraints map to a field and a message

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:**
- **The annotation:** the engine reads `-- rok:constraint <name> <field> "<message>"` anywhere in the schema dir, for a CHECK constraint or a trigger's `RAISE … USING CONSTRAINT = '<name>'`.
- **The table:** it generates a constraint table (name → field, message) in the data layer.
- **The templates' error mapping** (template half, after the gRPC chain):
  - A check violation on a mapped constraint is a 422 with that field's detail.
  - An unmapped one carries PostgreSQL's own message instead of "the value breaks the check …".
- **Errors:** an annotation naming an unknown field or constraint is a generation error.

**Blocked by:** None for the engine half; grpc-api-template/08 for the template half

**Status:** ready-for-agent

- [ ] The annotation parses; unknown field or constraint fails, naming it
- [ ] The generated table lists every annotated constraint
- [ ] Template: `offers_min_saving` comes back as a 422 on `price_tzs` with the message; a trigger's RAISE text reaches the client
