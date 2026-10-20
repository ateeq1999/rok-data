// Scaffolded once by rok-data-sqlc; this file is yours. It is never regenerated. See docs/guide/architecture.md.

//! `LabelsService`: the `labels` feature's business rules. Override any
//! [`LabelsServiceApi`] method here; the others keep their generated defaults.
//! A port this feature needs from another feature is a trait declared here
//! and implemented in `app.rs`.

use sqlx::PgPool;

use super::service_api::LabelsServiceApi;

/// The `labels` service, registered as app data by `register_services`.
pub struct LabelsService {
    /// The pool the service reads and writes through.
    pub pool: PgPool,
}

impl LabelsService {
    /// The service over `pool`, as `register_services` builds it. Give it
    /// what else it needs (a port) here.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        LabelsService { pool }
    }
}

impl LabelsServiceApi for LabelsService {
    fn pool(&self) -> &PgPool {
        &self.pool
    }
}
