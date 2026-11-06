# 67: Descriptions on nullable enum fields

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** utoipa writes a documented `Option<SomeEnum>` field as `oneOf: [null, {$ref, description}]`, with the description inside the `$ref` branch, so the TypeScript and Dart Clients lose the comment (a nullable plain field keeps it). When a property has no description of its own and is a `oneOf`/`anyOf` of `null` and one branch, the contract reader takes that branch's description, so every Client documents the field.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `NextPayout.status`'s comment reaches the TypeScript, Dart and Rust Clients (emission tests)
- [ ] A property's own description still wins
