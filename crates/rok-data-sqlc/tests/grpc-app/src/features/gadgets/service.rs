// Scaffolded once by rok-data-sqlc; this file is yours. It is never regenerated. See docs/guide/architecture.md.

//! `GadgetsService`: the `gadgets` feature's business rules. Override any
//! [`GadgetsServiceApi`] method here; the others keep their generated defaults.
//! A port this feature needs from another feature is a trait declared here
//! and implemented in `app.rs`.

use sqlx::PgPool;

use super::service_api::GadgetsServiceApi;

/// The `gadgets` service, registered as app data by `register_services`.
pub struct GadgetsService {
    /// The pool the service reads and writes through.
    pub pool: PgPool,
}

impl GadgetsService {
    /// The service over `pool`, as `register_services` builds it. Give it
    /// what else it needs (a port) here.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        GadgetsService { pool }
    }
}

impl GadgetsServiceApi for GadgetsService {
    fn pool(&self) -> &PgPool {
        &self.pool
    }
}
