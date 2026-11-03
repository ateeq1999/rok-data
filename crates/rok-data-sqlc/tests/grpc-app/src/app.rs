//! Builds the application: the gRPC services of the generated features.
//! This is the only wiring file.

use sqlx::PgPool;
use tonic::service::Routes;

use crate::features;

/// Every generated feature's service, over `pool`. The server adds the
/// interceptor that checks the credentials and puts the caller (and the
/// tenant) in each request's extensions, where the services read them.
#[must_use]
pub fn routes(pool: &PgPool) -> Routes {
    features::routes(pool)
}
