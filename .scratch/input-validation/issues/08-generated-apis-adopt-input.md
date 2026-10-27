# 08: Generated APIs adopt `#[derive(Input)]`

**What to build:** Generated request DTOs derive `Input` (not `validator::Validate`); the generated `exists`/`unique` pre-check code is removed in favor of the derive's database rules; OpenAPI body schemas come from `json_schema`; `[api.rules]` stays the generator's input.

**Blocked by:** 03, 04, 05, 06, 07

**Status:** ready-for-agent

- [ ] Wire answers byte-identical (templates' contract tests, fixture apps)
- [ ] Engine ADR recording the switch (next engine number)
- [ ] `validator` dropped from generated manifests where unused
