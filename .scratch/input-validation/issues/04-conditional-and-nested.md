# 04: Conditional rules and nested inputs

**What to build:** `required_if(field = "…", equals = …)`, `required_unless`, `required_with(other)` on `Option` fields (code `required`); `when = path` on a field; nested `Input` fields and `Vec<Input>` validated recursively with dotted/indexed paths; enum inputs validated per variant.

**Blocked by:** 02 (03 for nested database rules)

**Status:** done (2026-10-27) — nested fields are marked `#[rok(nested)]`; enum inputs wrap one input per variant; a nested input's custom database rules run through its own `validate` only

- [x] Facade tests for each conditional form, nested paths, arrays, enums
- [x] Nested database rules join the one batched statement
