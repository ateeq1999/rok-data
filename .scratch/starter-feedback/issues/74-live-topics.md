# 74: Live updates over Topics

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** a template Capability for server push, so a live board or a diner's Bill updates on news instead of polling.
- **Publish:** `publish(&mut tx, tenant, "orders.new", payload)` from any feature, sent with PostgreSQL `NOTIFY` on the feature's own transaction. PostgreSQL delivers it only when the transaction commits, so a notice is never sent for a rolled-back change, as the job queue's SMS already works. Every API instance sees it. It reuses the grpc template's `LISTEN/NOTIFY` shape.
- **Subscribe:** one server-sent-events route, `GET /api/v1/live?topics=…`, scoped to the caller's tenant and checked by a Guard (ticket 44). The browser's `EventSource` can't send a bearer header, so a short-lived ticket endpoint issues a token the route accepts.
- **Client helpers:**
  - **Web:** `subscribe(topics, onNotice)`, which calls `queryClient.invalidateQueries` for the keys a notice names.
  - **Dart:** a `Stream<Notice>` from the Client runtime.

  Both reconnect after a dropped connection.
- **Template Apps:** the Monorepo's mobile Bill and checkout, and the web kitchen board, switch from timers to Topics.
- **Bounded:** one listener per API instance fanning out to subscribers, with a bounded buffer. A slow client is dropped and reconnects.

The glossary's **Topic** names it. Karibu Pass polls the Bill every 10 s, the checkout every 3 s and the kitchen board too, each with its own timer (tickets 16, 23).

**Blocked by:** 44 (done)

**Status:** ready-for-agent

- [ ] A notice published in one request reaches a subscriber's stream, and none is sent when its transaction rolls back (tests against PostgreSQL)
- [ ] A subscriber sees only its tenant's Topics; a caller the Guard refuses gets 403
- [ ] The web helper invalidates the named queries, and the Dart stream reconnects (tests)
