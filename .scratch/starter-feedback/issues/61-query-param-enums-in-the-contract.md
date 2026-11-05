# 61: Query-parameter enums reach the contract

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** generated features register in their OpenAPI document every schema their query-parameter (`IntoParams`) structs use, so an enum filter's `$ref` always resolves. For hand-written code, `rok gen`'s "`$ref to X, which #/components/schemas does not define`" error adds where to register it: `components(schemas(<path>::X))` in `openapi.rs` (or the feature's `ApiDoc` once ticket 41 lands), with the type's path when it can be found in `src/`.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A generated feature with an enum query filter produces a valid contract
- [ ] A hand-written unregistered enum fails naming the line to add
