# 18: The TypeScript Client's types are zod schemas, and calls check responses

Spec: `docs/specs/starter-feedback.md` (Typed web data) · engine ADR 0027 (amended 2026-11-03)

**What to build:** in the engine's TypeScript Client emitter:
- **Schemas:** each component schema becomes `export const <name>Schema = z.object(...)` and `export type <Name> = z.infer<typeof <name>Schema>`; `oneOf` becomes `z.union`.
- **The server's rules:** the OpenAPI reader keeps `minLength`/`maxLength`, `minimum`/`maximum`, `pattern` and `format` (`email`, `uuid`, `date-time`) for TypeScript, and they become zod rules.
- **Calls:** each call parses its response with the response schema and throws a `ContractError` (operation, zod issue paths) on a mismatch, apart from `ApiError`. Request bodies are sent unchecked.
- **Header:** the Client's header names the zod major it targets (4).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Emission tests: object, nullable/optional, enum, array, `oneOf` union, each constraint
- [ ] A response that doesn't match throws `ContractError`; unknown keys are stripped, not rejected
- [ ] Type names unchanged, so existing imports compile
- [ ] Dart and Rust Clients unchanged (their emission tests still pass)
