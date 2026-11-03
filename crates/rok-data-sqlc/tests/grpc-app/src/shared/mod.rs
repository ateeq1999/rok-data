//! Tools with no business meaning, shared by the features. `shared/` never
//! imports `features/`.
//!
//! `rok sqlc generate` regenerates `grpc`, `pagination`, `response` and
//! `validation`; don't edit them. `auth` is yours, like any tool you add
//! here (mail, storage, a queue), each behind a trait.

pub mod auth;
pub mod grpc;
pub mod pagination;
pub mod response;
pub mod validation;
