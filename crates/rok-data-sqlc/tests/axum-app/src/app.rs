//! Builds the application: the router the generated features, the shared
//! tools and the `OpenAPI` document are served by. This is the only wiring
//! file.

use axum::Router;
use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::get;
use sqlx::PgPool;

use crate::features;
use crate::openapi;
use crate::shared::auth::JwtKeys;
use crate::shared::health;

/// Mounts the whole application: every feature's routes, `/health/live` and
/// `/health/ready`, and the `OpenAPI` document at `/api-docs/openapi.json`.
/// The JWT keys travel in the request's extensions, where the auth
/// extractor finds them.
pub fn app(pool: &PgPool, keys: &JwtKeys) -> Router {
    let keys = keys.clone();
    Router::new()
        .merge(features::router(pool))
        .merge(health::routes(pool))
        .route("/api-docs/openapi.json", get(document))
        .layer(middleware::from_fn(move |mut request: Request, next: Next| {
            let keys = keys.clone();
            async move {
                request.extensions_mut().insert(keys);
                next.run(request).await
            }
        }))
}

/// The `OpenAPI` document, JSON-wrapped for the `/api-docs/openapi.json`
/// route. The document itself lives framework-neutral in
/// [`crate::openapi`]; the framework type belongs in this wiring file.
async fn document() -> impl IntoResponse {
    axum::Json(openapi::document())
}
