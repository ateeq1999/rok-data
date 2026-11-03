# 23: Errors show their causes

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** the CLI prints every error with its whole chain (`{:#}`), so `failed to parse SQL schema` carries `<file>:<line>: invalid PostgreSQL SQL syntax: <parser message>`. The `--json` error body carries the chain too.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A schema with a syntax error names the file, the line and the parser's message
- [ ] Existing error-output tests updated deliberately, not loosened
