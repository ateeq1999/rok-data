# 08: Generated APIs adopt `#[derive(Input)]`

**What to build:** Generated request DTOs derive `Input` (not `validator::Validate`); the generated `exists`/`unique` pre-check code is removed in favor of the derive's database rules; OpenAPI body schemas come from `json_schema`; `[api.rules]` stays the generator's input.

**Blocked by:** 03, 04, 05, 06, 07

**Status:** ready-for-agent

- [ ] Wire answers byte-identical (templates' contract tests, fixture apps)
- [ ] Engine ADR recording the switch (next engine number)
- [ ] `validator` dropped from generated manifests where unused

**Decisions (2026-10-27/28, with the user):**
- `params.value` is dropped from the generated answers when they adopt the derive (option a): a documented wire change.
- OpenAPI keeps `utoipa` (the engine's existing `ToSchema` schemas); the "OpenAPI from `json_schema`" item is dropped.
- The caller-scoped `exists` / `unique` pre-checks move into database rules through a **validation context** (option a): `#[rok(context = Caller)]`, `validate_with(&db, &caller)`, `scope = fn(&Self, &Caller)`.

**Progress:**
- [x] Part 1: validation context in the runtime and the derive (`type Context`, `validate_with`, context-aware scopes / custom rules / record rule, nested and enum inputs share it).
- [ ] Part 2: generated DTOs derive `Input`, the generated pre-checks become context-scoped database rules, `validator` leaves the generated manifests.

**Decisions (2026-10-28, with the user):**
- Caller-scoped `exists` / `unique` need who is calling: inputs gain a **validation context** (`#[rok(context = Caller)]`, `validate_with(&db, &ctx)`, scopes and custom rules receive `&ctx`). Done (first part).
- OpenAPI keeps the engine's `utoipa` schemas; `json_schema()` does not replace them.
- Generated answers drop `value` from `params` (option (a) of ticket 05's note): a documented wire change.
