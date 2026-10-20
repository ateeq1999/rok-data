//! The `OpenAPI` document the application serves at `/api-docs/openapi.json`:
//! the generated features' `GeneratedApiDoc`, merged into [`ApiDoc`], which
//! lists your hand-written paths and schemas.

use axum::Json;
use utoipa::OpenApi;

use crate::features::GeneratedApiDoc;

/// Your hand-written paths and schemas: list a handler carrying
/// `#[utoipa::path]` in `paths(...)`, and a type deriving `ToSchema` that
/// no path refers to in `components(schemas(...))`. Where yours and the
/// generated document name the same path or schema, yours wins.
#[derive(OpenApi)]
#[openapi(paths(), components(schemas()))]
pub struct ApiDoc;

/// The whole document: [`ApiDoc`] with every generated path and schema
/// merged in, titled and versioned after the crate.
#[must_use]
pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut doc = ApiDoc::openapi();
    doc.merge(GeneratedApiDoc::openapi());
    doc.info.title = env!("CARGO_PKG_NAME").to_owned();
    doc.info.version = env!("CARGO_PKG_VERSION").to_owned();
    doc
}

/// The document, as the application serves it.
pub async fn document() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
}
