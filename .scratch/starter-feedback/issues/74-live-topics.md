# 74: Live updates over Topics

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** a template Capability for server push, so a live board refetches on news instead of polling.
- **Publish:** `publish(tenant, "orders.new", payload)` from any feature, sent with PostgreSQL `NOTIFY`, so every API instance sees it. It reuses the grpc template's `LISTEN/NOTIFY` shape.
- **Subscribe:** one server-sent-events route, `GET /api/v1/live?topics=…`, scoped to the caller's tenant and checked by a Guard (ticket 44). The browser's `EventSource` can't send a bearer header, so a short-lived ticket endpoint issues a token the route accepts.
- **Web helper:** `subscribe(topics, onNotice)` calls `queryClient.invalidateQueries` for the keys a notice names.
- **Bounded:** one listener per API instance fanning out to subscribers, with a bounded buffer. A slow client is dropped, and the browser reconnects.

The glossary's **Topic** names it.

**Blocked by:** 44, grpc-api-template/08 (the gRPC Template shipped)

**Status:** blocked

- [ ] A notice published in one request reaches a subscriber's stream (test against PostgreSQL)
- [ ] A subscriber sees only its tenant's Topics; a caller the Guard refuses gets 403
- [ ] The web helper invalidates the named queries (web test)
