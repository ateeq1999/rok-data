# 05: Restore the API layer's names (the data-layer rename stays)

**What to build:** Commit `4f1c39c` renamed the API layer along with the data layer: handler and service-trait methods `update→patch`, `remove→destroy`, OpenAPI `operationId`s, the `api.skip` key `delete→destroy`. 0.3.0 handler story 4 and D6 require the API features and OpenAPI document to stay unchanged. A user's `service.rs` override (`fn remove`) and every OpenAPI client keyed on `operationId` break today. Restore the API-layer names; the data-layer verbs keep `patch`/`destroy`.

**Blocked by:** 01

**Status:** done

- [x] Handler and service-trait method names, `operationId`s and the `api.skip` grammar match 0.2.0 again
- [x] ~~`api.skip` accepts `destroy` as an alias~~ — dropped: `destroy` never shipped (only unreleased `main` had it), and serde lists an alias in its "expected one of" error, which would advertise the wrong spelling to every user who mistypes a name. The 0.2.0 name `remove` is the only spelling.
- [x] `docs/cli-reference.md` needed no change (it never carried the 0.3.0 names); the engine README, configuring and troubleshooting guides, and the migration guide are corrected
- [x] OpenAPI snapshot diff against 0.2.0 shows no operation renames
- [x] CLI suites and fixture compile gates green
