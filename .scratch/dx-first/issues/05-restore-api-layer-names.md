# 05: Restore the API layer's names (the data-layer rename stays)

**What to build:** Commit `4f1c39c` renamed the API layer along with the data layer: handler and service-trait methods `update→patch`, `remove→destroy`, OpenAPI `operationId`s, the `api.skip` key `delete→destroy`. 0.3.0 handler story 4 and D6 require the API features and OpenAPI document to stay unchanged. A user's `service.rs` override (`fn remove`) and every OpenAPI client keyed on `operationId` break today. Restore the API-layer names; the data-layer verbs keep `patch`/`destroy`.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] Handler and service-trait method names, `operationId`s and the `api.skip` grammar match 0.2.0 again
- [ ] `api.skip` accepts `destroy` as an alias for `delete` (whoever already wrote it keeps working)
- [ ] `docs/cli-reference.md` updated in the same change
- [ ] OpenAPI snapshot diff against 0.2.0 shows no operation renames
- [ ] CLI suites and fixture compile gates green
