//! The Clients of a monorepo (workspace ADR 0027): typed callers of the
//! API's contract, emitted from its `openapi.json` rather than from the
//! engine's model — the hand-written routes (auth, two-factor, uploads)
//! exist only in the document. [`openapi`] reads the subset utoipa emits
//! into the model every emitter shares; [`ts`] emits the TypeScript
//! Client from it. [`Project`] is where a monorepo's `rok.toml` keeps the
//! contract and the Clients, and [`generate`] writes (or checks) them with
//! the engine's generated-file rules: `init` writes the missing files,
//! `rok gen` re-exports the contract and rewrites them, `rok gen check`
//! reports their drift (engine ADR 0027).

pub mod openapi;
mod project;
pub mod ts;
mod write;

pub use project::{CLIENTS, Client, Project};
pub use write::{Mode, Outcome, emit_client, generate};
