//! Builds the application: its state, the generated features' services and
//! routes, health, and the `OpenAPI` document. This is the only wiring file;
//! the ports features declare for each other are implemented here too.

use axum::Router;
use axum::extract::Request;
use axum::middleware::{self, Next};
use sqlx::PgPool;
use tokio::net::TcpListener;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::Config;
use crate::db::pool::init_pool;
use crate::features;
use crate::openapi::document;
use crate::shared::auth::JwtKeys;
use crate::shared::health;

/// Connects the pool, then serves the application on `config.bind_addr`
/// until the server stops.
pub async fn run(config: Config) -> std::io::Result<()> {
    let pool = init_pool(&config.database_url, &config.pool)
        .await
        .map_err(std::io::Error::other)?;
    let keys = JwtKeys::new(&config.jwt_secret);
    let listener = TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "listening");
    axum::serve(listener, app(&pool, &keys))
        .await
        .map_err(std::io::Error::other)
}

/// Mounts the whole application: every feature's routes, `/health/live` and
/// `/health/ready`, the `OpenAPI` document at `/api-docs/openapi.json` with
/// Swagger UI at `/docs/`, and the JWT keys in the request's extensions,
/// where the auth extractor finds them. Tests build the same application
/// with `app(&pool, &keys)`.
pub fn app(pool: &PgPool, keys: &JwtKeys) -> Router {
    let keys = keys.clone();
    Router::new()
        .merge(features::router(pool))
        .merge(health::routes(pool))
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", document()))
        .layer(middleware::from_fn(
            move |mut request: Request, next: Next| {
                let keys = keys.clone();
                async move {
                    request.extensions_mut().insert(keys);
                    next.run(request).await
                }
            },
        ))
}
