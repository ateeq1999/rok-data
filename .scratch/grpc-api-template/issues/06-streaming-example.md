# 06: The streaming example

Spec: `docs/specs/grpc-api-template.md`

**What to build:** one hand-written, scaffolded (user-owned) server-streaming RPC in the `grpc-api` template, fed by PostgreSQL `LISTEN/NOTIFY`: a client subscribes and receives an event per notification. Streaming is not generated.

**Blocked by:** 04

**Status:** blocked

- [ ] The RPC lives in a hand-written `.proto` importing the generated messages
- [ ] A test subscribes, triggers a `NOTIFY`, and receives the event (skips with a note without a database)
