//! The shared tools that hold axum's own types (engine ADR 0016): health's routes,
//! the tenant, and the validated bodies. actix-web's are
//! [`super::actix`].

pub(crate) const HEALTH: &str = r#"//! `GET /health/live` and `GET /health/ready`, outside the API prefix.

use axum::Json;
use axum::Router;
use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
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
/// the pool from the `Extension<PgPool>`.
pub fn routes(pool: &PgPool) -> Router {
    Router::new()
        .route("/health/live", get(liveness))
        .route("/health/ready", get(readiness))
        .layer(Extension(pool.clone()))
}

/// `200 {"status": "ok"}` while the server answers at all.
pub async fn liveness() -> Response {
    Json(Status { status: "ok" }).into_response()
}

/// `200 {"status": "ready"}` when the database answers, otherwise `503`
/// with `SERVICE_UNAVAILABLE`.
pub async fn readiness(Extension(pool): Extension<PgPool>) -> Response {
    match check_health(&pool).await {
        Ok(()) => Json(Status { status: "ready" }).into_response(),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorBody::new(
                SERVICE_UNAVAILABLE,
                "the database is not reachable",
            )),
        )
            .into_response(),
    }
}
"#;

pub(crate) const TENANT: &str = r#"//! [`Tenant`]: the tenant of a request, as the auth middleware found it.

use std::ops::Deref;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::error::AppError;

/// The tenant of a request, which the auth middleware stored in the
/// request's extensions. Handlers of tenant-scoped tables take one. A
/// request without one is a wiring bug, not the caller's fault: a `500`
/// with `internal_error`.
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

impl<T: Clone + Send + Sync + 'static> FromRequestParts<()> for Tenant<T> {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &()) -> Result<Self, Self::Rejection> {
        let tenant = parts.extensions.get::<T>().cloned();
        tenant.map(Tenant).ok_or_else(|| {
            AppError::Internal(
                format!(
                    "no `{}` tenant in the request extensions: is the middleware that sets it mounted?",
                    std::any::type_name::<T>()
                )
                .into(),
            )
        })
    }
}
"#;

pub(crate) const VALIDATION: &str = r#"//! Reading a request: validated JSON bodies, the wire standard's `400` for a
//! malformed path or query, a versioned row's `If-Match`, and a
//! many-to-many sync's list of keys.
//!
//! A handler takes [`ValidatedJson`], which checks the body's rules and
//! answers a bad body with a [`BodyRejection`] in the wire standard's error
//! body. A malformed path or query is answered the same way, by [`Path`] and
//! [`Query`].

use std::ops::Deref;
use std::str::FromStr;

use axum::extract::FromRequestParts;
use axum::http::header;
use axum::http::request::Parts;

use crate::error::AppError;

pub use crate::db::http::BodyRejection;
pub use crate::db::http::axum::ValidatedJson;

/// The path of a request, deserialized into `T`: a path that doesn't parse,
/// such as `/notes/not-a-uuid`, is a `400` with `bad_request`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Path<T>(pub T);

impl<T> Path<T> {
    /// The path's value.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for Path<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T, S> FromRequestParts<S> for Path<T>
where
    T: serde::de::DeserializeOwned + Send + Sync,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let path = axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(|rejection| AppError::BadRequest(rejection.body_text()))?;
        Ok(Path(path.0))
    }
}

/// The query of a request, deserialized into `T`: a query string that
/// doesn't parse, such as `?page=abc`, is a `400` with `bad_request`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Query<T>(pub T);

impl<T> Query<T> {
    /// The query's value.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for Query<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T, S> FromRequestParts<S> for Query<T>
where
    T: serde::de::DeserializeOwned + Send + Sync,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let query = axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|rejection| AppError::BadRequest(rejection.body_text()))?;
        Ok(Query(query.0))
    }
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

impl<T: FromStr> FromRequestParts<()> for IfMatch<T> {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &()) -> Result<Self, Self::Rejection> {
        if_match(parts).map(IfMatch)
    }
}

/// The version `parts`' `If-Match` holds.
fn if_match<T: FromStr>(parts: &Parts) -> Result<T, AppError> {
    let value = parts.headers.get(header::IF_MATCH).ok_or_else(|| {
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
