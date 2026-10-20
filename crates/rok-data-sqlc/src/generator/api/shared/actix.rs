//! The shared tools that hold actix-web's own types (ADR 0016): health's
//! routes, the tenant, and the validated bodies. axum's are
//! [`super::axum`].

pub(crate) const HEALTH: &str = r#"//! `GET /health/live` and `GET /health/ready`, outside the API prefix.

use actix_web::{HttpResponse, web};
use serde::Serialize;
use sqlx::PgPool;

use super::response::ErrorBody;
use crate::db::pool::check_health;

/// The `code` of `/health/ready`'s `503`.
pub const SERVICE_UNAVAILABLE: &str = "service_unavailable";

/// A healthy answer: `{"status": "ok"}` or `{"status": "ready"}`.
#[derive(Debug, Serialize)]
struct Status {
    status: &'static str,
}

/// Mounts `GET /health/live` and `GET /health/ready`. `/health/ready` reads
/// the pool from the `web::Data<PgPool>` app data.
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/health/live", web::get().to(liveness))
        .route("/health/ready", web::get().to(readiness));
}

/// `200 {"status": "ok"}` while the server answers at all.
pub async fn liveness() -> HttpResponse {
    HttpResponse::Ok().json(Status { status: "ok" })
}

/// `200 {"status": "ready"}` when the database answers, otherwise `503`
/// with `SERVICE_UNAVAILABLE`.
pub async fn readiness(pool: web::Data<PgPool>) -> HttpResponse {
    match check_health(&pool).await {
        Ok(()) => HttpResponse::Ok().json(Status { status: "ready" }),
        Err(_) => HttpResponse::ServiceUnavailable().json(ErrorBody::new(
            SERVICE_UNAVAILABLE,
            "the database is not reachable",
        )),
    }
}
"#;

pub(crate) const TENANT: &str = r#"//! [`Tenant`]: the tenant of a request, as the auth middleware found it.

use std::future::{Ready, ready};
use std::ops::Deref;

use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpMessage, HttpRequest};

use crate::error::AppError;

/// The tenant of a request, which the auth middleware stored in the
/// request's extensions (`req.extensions_mut().insert(tenant)`). Handlers of
/// tenant-scoped tables take one. A request without one is a wiring bug, not
/// the caller's fault: a `500` with `internal_error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tenant<T>(pub T);

impl<T> Tenant<T> {
    /// The tenant.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for Tenant<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Clone + 'static> FromRequest for Tenant<T> {
    type Error = AppError;
    type Future = Ready<Result<Self, AppError>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let tenant = req.extensions().get::<T>().cloned();
        ready(tenant.map(Tenant).ok_or_else(|| {
            AppError::Internal(
                format!(
                    "no `{}` tenant in the request extensions: is the middleware that sets it mounted?",
                    std::any::type_name::<T>()
                )
                .into(),
            )
        }))
    }
}
"#;

pub(crate) const VALIDATION: &str = r#"//! Reading a request: validated JSON bodies, the wire standard's `400` for a
//! malformed path or query, a versioned row's `If-Match`, and a
//! many-to-many sync's list of keys.
//!
//! A handler takes [`ValidatedJson`], which checks the body's rules and
//! answers a bad body with a [`BodyRejection`] in the wire standard's error
//! body. Every feature's scope uses [`path_config`] and [`query_config`].

use std::future::{Ready, ready};
use std::str::FromStr;

use actix_web::dev::Payload;
use actix_web::http::header;
use actix_web::{FromRequest, HttpRequest, web};

use crate::error::AppError;

pub use crate::db::http::BodyRejection;
pub use crate::db::http::actix_web::ValidatedJson;

/// The `web::PathConfig` of every feature's scope: a path that doesn't
/// parse, such as `/notes/not-a-uuid`, is a `400` with `bad_request`.
#[must_use]
pub fn path_config() -> web::PathConfig {
    web::PathConfig::default()
        .error_handler(|error, _| AppError::BadRequest(error.to_string()).into())
}

/// The `web::QueryConfig` of every feature's scope: a query string that
/// doesn't parse, such as `?page=abc`, is a `400` with `bad_request`.
#[must_use]
pub fn query_config() -> web::QueryConfig {
    web::QueryConfig::default()
        .error_handler(|error, _| AppError::BadRequest(error.to_string()).into())
}

/// The version an `If-Match` header holds, `"<version>"`: the `ETag` a
/// versioned row was read with. A handler updating such a row takes it, so
/// a request without one is a `428` with `precondition_required`, and one
/// holding no version is a `412` with `version_conflict`, as it can't
/// match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IfMatch<T>(pub T);

impl<T> IfMatch<T> {
    /// The version.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: FromStr> FromRequest for IfMatch<T> {
    type Error = AppError;
    type Future = Ready<Result<Self, AppError>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(if_match(req).map(IfMatch))
    }
}

/// The version `req`'s `If-Match` holds.
fn if_match<T: FromStr>(req: &HttpRequest) -> Result<T, AppError> {
    let value = req.headers().get(header::IF_MATCH).ok_or_else(|| {
        AppError::PreconditionRequired("`If-Match` must hold the row's `ETag`".to_owned())
    })?;
    let version = value
        .to_str()
        .ok()
        .and_then(|value| value.trim().strip_prefix('"')?.strip_suffix('"'))
        .and_then(|version| version.parse().ok());
    version.ok_or_else(|| {
        AppError::VersionConflict("`If-Match` holds no version of the row".to_owned())
    })
}

/// The body of a many-to-many sync, `{"ids": [...]}`: the keys of the
/// related rows to pair with, and no others.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, validator::Validate)]
pub struct IdList<T> {
    /// The related rows' keys.
    pub ids: Vec<T>,
}
"#;
