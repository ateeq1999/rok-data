//! The `stats` routes, under `/api/v1/stats`.

use actix_web::web;

use super::handlers;
use crate::shared::validation::resource;

/// Mounts `GET /api/v1/stats`: a resource, not a scope, so the generated
/// features' routes under `/api/v1` keep answering.
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(resource("/api/v1/stats").route(web::get().to(handlers::summary)));
}
