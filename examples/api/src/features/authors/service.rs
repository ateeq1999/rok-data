// Part of the hand-written half of the example: this file is yours, and `rok gen` never rewrites it.

//! `AuthorsService`: the `authors` feature's business rules. Override any
//! [`AuthorsServiceApi`] method here; the others keep their generated defaults.
//! A port this feature needs from another feature is a trait declared here
//! and implemented in `app.rs`.

use sqlx::PgPool;

use super::service_api::AuthorsServiceApi;

/// The `authors` service, mounted by `router`.
pub struct AuthorsService {
    /// The pool the service reads and writes through.
    pub pool: PgPool,
}

impl AuthorsService {
    /// The service over `pool`, as `router` builds it. Give it
    /// what else it needs (a port) here.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        AuthorsService { pool }
    }
}

impl AuthorsServiceApi for AuthorsService {
    fn pool(&self) -> &PgPool {
        &self.pool
    }
}
