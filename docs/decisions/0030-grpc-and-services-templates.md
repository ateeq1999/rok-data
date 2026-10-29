# ADR 0030: a gRPC Template, and a services Template whose brokers are Integrations

Date: 2026-10-29 · Status: accepted

## Decision

- **`grpc-api` is a Template of its own**, tonic only. It shares config,
  db, jobs and the Capabilities with `axum-api` as Shared files, and its
  generated handlers call the same `features/*/service.rs` the HTTP
  handlers do, so serving gRPC from `axum-api` can later be a `grpc`
  Integration. Per Resource the engine generates unary
  `Get/List/Create/Update/Delete` (`List` carrying `Page`'s fields) and
  the `.proto` under `proto/`, `@generated` and drift-checked; user RPCs
  live in hand-written `.proto` files that import the generated
  messages. Streaming is one hand-written `LISTEN/NOTIFY` example;
  `tonic-health` and `tonic-reflection` are built in; grpc-web is off.
  The generated-code shape gets its engine ADR when it is built.

- **gRPC verifies auth, it does not issue it**: an interceptor checks the
  bearer tokens and API keys `axum-api` issues. Login, registration, 2FA
  and passkeys stay HTTP.

- **gRPC Clients are protobuf's**: `tonic-build` for Rust, the standard
  protoc plugins for other languages. ADR 0027's own generators are for
  OpenAPI, whose generators we cannot shape; protobuf codegen is
  uniform already.

- **A `services` Template**: a **Gateway** (axum; HTTP to gRPC, issues
  auth, serves the OpenAPI document the Clients follow), Services built
  from `grpc-api` (`accounts` publishes `user.registered`,
  `notifications` consumes it and sends mail through jobs), a shared
  `events` crate, one database per Service on the same Server (ADR
  0028), a root Cargo workspace, and `docker-compose.yml`.

- **Events ride the outbox.** An `Event` is typed (name, version, serde
  payload), published on the caller's transaction like a job (ADR 0024),
  delivered at least once, and deduplicated by the consumer's `inbox`
  table. The built-in transport is Postgres (outbox + `LISTEN/NOTIFY`), a
  Capability switched by env; RabbitMQ, Redis Streams and NATS are
  Integrations (ADR 0029) behind the same publisher / subscriber trait.
  RPC between Services is gRPC.

- Web and mobile Apps on a services project are a Blueprint combining
  Templates, not a `services` flag.

## Considered Options

- **One Template per broker**: the same services multiplied by brokers,
  where the broker is the only difference — the case Integrations exist
  for.
- **Redis pub/sub**: at most once; a consumer that is down loses the
  event, which breaks the outbox's guarantee. Redis Streams keep the
  vendor with consumer groups and acknowledgements.
- **Auth flows as RPCs**: a second copy of the largest hand-written part
  of the templates.

## Consequences

- PostgreSQL-only still holds for data: a broker is never required, and
  one that is added carries events, not rows.
