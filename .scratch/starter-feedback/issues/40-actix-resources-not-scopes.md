# 40: Actix features mount resources, not scopes, in an order that matches

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** in the engine's actix routes and `check-arch`:
- **Resources, not scopes:** a generated feature uses `web::resource` only, never `web::scope`, so a feature can't answer 404 for another feature's paths under a shared prefix.
- **Fixed paths first:** within a feature, the generated routes register fixed paths before parameterised ones (`/checkout/tickets` before `/checkout/{id}`). An actix resource that matches the path but not the method answers 405 and doesn't fall through to a later resource.
- **check-arch:** it warns when a hand-written feature's `web::scope` prefix covers another feature's routes, and when a hand-written fixed path is registered after a parameterised path that matches it.
- **Docs:** the generated-routes docs explain both actix behaviours and the order rule for hand-written features.

Karibu Pass met both: a `/partner` scope hiding `/partner/{id}/offers` (ticket 05), and `POST /checkout/tickets` answering 405 behind `GET /checkout/{id}` (ticket 19).

**Blocked by:** grpc-api-template/08 (the gRPC Template shipped)

**Status:** done

- [x] Two features under one prefix (`/partner` and `/partner/{id}/offers`) both answer, in either mount order (test)
- [x] A fixed path and a parameterised one in a feature both answer each method (test)
- [x] check-arch warns on a covering scope and on a fixed path behind a matching parameterised one

**Done (sf/40):** the engine's actix `routes.rs` mounts one
`shared::validation::resource` per path (a `web::resource` with the `400`
path and query configs), ordered so a fixed segment comes before a
`{parameter}`; `check-arch` warns `covering-scope` and `route-order`
(warnings print on stderr and never fail the run); the architecture and
customizing guides explain both actix behaviours and the order rule.
Tests: `tests/api-app/tests/mounting.rs` (both boxes, against PostgreSQL),
`api_features::a_feature_mounts_resources_fixed_paths_first`, the two
`check_arch` unit tests and `tests/cli/check_arch_mounts.rs`.

**Follow-up in rok-templates:** the generated actix `routes.rs` and
`shared/validation.rs` change, so the actix templates need their
`rok-data.pin` bumped and regenerating once this ships.
