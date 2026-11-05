# 41: `rok add feature <name>`

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it) · workspace ADR 0029 (Integration points)

**What to build:**
- **The command:** `rok add feature <name>` scaffolds a hand-written feature (`mod.rs`, `routes.rs`, `handlers.rs`, `service.rs`, `dto.rs`, for the App's framework).
- **Registration:** it registers the feature at the template's Integration points: `features/mod.rs`, the service and route mount in the router, and the OpenAPI document.
- **OpenAPI:** each feature contributes one nested `ApiDoc` (utoipa `nest`), so its OpenAPI registration is one line.
- **Errors:** a feature that already exists is an error.
- **Payload:** `-j` input works as for every command.

**Blocked by:** None for the command; the templates' Integration points and per-feature `ApiDoc` land with grpc-api-template/08

**Status:** ready-for-agent

- [ ] A scaffolded feature compiles and is served and documented with no further edits (gate on a template)
- [ ] Running it twice fails, naming the feature
