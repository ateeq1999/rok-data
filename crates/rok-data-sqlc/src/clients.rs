//! The Clients of a monorepo (workspace ADR 0027): typed callers of the
//! API's contract, emitted from its `openapi.json` rather than from the
//! engine's model — the hand-written routes (auth, two-factor, uploads)
//! exist only in the document. [`openapi`] reads the subset utoipa emits
//! into the model every emitter shares.

pub mod openapi;
