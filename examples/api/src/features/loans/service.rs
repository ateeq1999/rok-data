// Part of the hand-written half of the example: this file is yours, and `rok gen` never rewrites it.

//! `LoansService`: the `loans` feature's business rules. Override any
//! [`LoansServiceApi`] method here; the others keep their generated defaults.
//! A port this feature needs from another feature is a trait declared here
//! and implemented in `app.rs`.

use sqlx::PgPool;

use super::service_api::LoansServiceApi;

/// The `loans` service, mounted by `router`.
pub struct LoansService {
    /// The pool the service reads and writes through.
    pub pool: PgPool,
}

impl LoansService {
    /// The service over `pool`, as `router` builds it. Give it
    /// what else it needs (a port) here.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        LoansService { pool }
    }
}

impl LoansServiceApi for LoansService {
    fn pool(&self) -> &PgPool {
        &self.pool
    }
}
