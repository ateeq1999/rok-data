# 97: Validation rules reach the contract, and the TypeScript Client carries them

Spec: `docs/specs/starter-feedback.md` (Typed web data) · engine ADR 0027 (amended 2026-11-28)

**What to build:**
- **The derive publishes its rules.** `rok_data::Input` exposes each field's rules for the API document: the rules, their messages, and the field's transforms. This covers every body and query-parameter struct, generated or hand-written.
- **The document applies them.** A `utoipa::Modify`, registered by the generated `ApiDoc` and usable from a hand-written one, writes them into each component schema and parameter:
  - `length` → `minLength`/`maxLength`
  - `range` → `minimum`/`maximum`
  - `email`, `url`, `uuid`, `ip` → `format`
  - `one_of` → `enum`
  - `starts_with`, `ends_with`, `alpha`, `ascii`, `hex_code` → `pattern`
  - `x-rok-messages`: each keyword's message
  - `x-rok-transforms`: `trim`/`lowercase`, in order

  `iban`, `credit_card`, `jwt`, `coordinates`, `validate_with` and the database checks stay server-only.
- **One way in.** The generator stops writing its own `#[schema(format, pattern, min_length, max_length)]` copy (`generator/api/rules.rs` `push_schema`), so generated and hand-written structs reach the contract the same way.
- **The reader and the TypeScript emitter:**
  - The reader keeps `x-rok-messages` and `x-rok-transforms`.
  - The emitter writes transforms first, then each rule with its message: `z.string().trim().toLowerCase().min(8, "password must be 8 to 128 characters")`.
  - Query-parameter objects get the same rules.
  - Request bodies stay unchecked at runtime.
- **Dart and Rust Clients:** unchanged.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A hand-written body's `#[rok(validate(…))]` rules, messages and transforms appear in the API document (test)
- [ ] A generated create body's rules appear the same way, with no `#[schema(…)]` copy in the generated code, and its `range` becomes `minimum`/`maximum`
- [ ] A query-parameter struct's rules appear on its parameters
- [ ] Server-only rules don't appear as keywords
- [ ] The TypeScript Client emits each rule with its message, and the transforms (emission tests)
- [ ] Dart and Rust emission tests unchanged
