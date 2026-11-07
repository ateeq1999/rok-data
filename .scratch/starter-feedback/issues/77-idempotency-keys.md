# 77: Idempotency keys for calls that move money

Spec: `docs/specs/starter-feedback.md` (Bugs found building on it)

**What to build:** a retry after a dropped connection is safe end to end.
- **Server (template Capability):** routes that declare an `Idempotency-Key` header parameter in their OpenAPI go through an idempotency layer.
  - **First request:** its answer is stored per caller, key and route, in a table with a TTL.
  - **A repeat with the same key and body:** gets the stored answer without running again.
  - **The same key with a different body:** gets 422.
- **Clients (engine):** for an operation with that header, the generated call fills a fresh UUID when the caller gives none and lets a retry reuse it: Dart `RequestOptions(idempotencyKey: …)`, TypeScript `{ idempotencyKey }`, Rust an argument. This builds on ticket 66's header support.
- **Recipe:** "safe retry after a dropped connection", using Karibu's `POST /checkout/{id}/pay`.

Karibu Pass's mobile App gets `ApiException(0, network_error)` with no way to know whether the payment prompt went out, so it never retries `pay` (ticket 22).

**Blocked by:** template tickets 44 and 11 landing (the template batch in progress)

**Status:** blocked

- [ ] A repeated `pay` with the same key starts one payment and returns the first answer (test against PostgreSQL)
- [ ] The same key with another body is refused
- [ ] Each Client fills and reuses the key (emission tests); the recipe compiles
