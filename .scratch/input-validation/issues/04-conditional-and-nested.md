# 04: Conditional rules and nested inputs

**What to build:** `required_if(field = "…", equals = …)`, `required_unless`, `required_with(other)` on `Option` fields (code `required`); `when = path` on a field; nested `Input` fields and `Vec<Input>` validated recursively with dotted/indexed paths; enum inputs validated per variant.

**Blocked by:** 02 (03 for nested database rules)

**Status:** ready-for-agent

- [ ] Facade tests for each conditional form, nested paths, arrays, enums
- [ ] Nested database rules join the one batched statement
