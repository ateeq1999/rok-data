// Scaffolded once by rok-data-sqlc; this file is yours. It is never regenerated. See docs/guide/architecture.md.

//! Who is calling: the [`AuthUser`] and [`AdminUser`] extractors.
//!
//! Each verifies the HS256 JWT from `Authorization: Bearer <token>` with the
//! [`JwtKeys`] that `app.rs` puts in the request's extensions, then loads
//! the account with one query (`db::queries::session::session_state`, on
//! the `PgPool` in the same extensions): its `token_version`, whether it is
//! active, and its role. A missing or invalid token, a soft-deleted or
//! disabled account, or a token issued before the last `token_version` bump
//! is a `401`. The role comes from the database, not the token, so a
//! promotion or demotion applies on the next request; a caller who is not
//! an admin is a `403` for `AdminUser`. Both answer through [`AppError`].
//!
//! Reading from the extensions, not the router's state, lets the same
//! extractors serve the generated routes (stateless, `features::router`)
//! and the hand-written ones (`Router<AppState>`).

use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, header};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::queries::session::{SessionStateRow, session_state};
pub use crate::db::types::Role;
use crate::error::AppError;
pub use crate::shared::security::jwt::{Claims, JwtKeys};

/// A signed-in caller with a live, active account. Take it in a handler to
/// require one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthUser {
    /// The token's `sub`.
    pub id: Uuid,
    /// The account's role, from the database.
    pub role: Role,
}

/// A signed-in caller whose account has the `admin` role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminUser(pub AuthUser);

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, AppError> {
        let wiring = |what: &str| {
            AppError::Internal(format!("{what} is not in the request extensions").into())
        };
        let keys = parts
            .extensions
            .get::<Arc<JwtKeys>>()
            .ok_or_else(|| wiring("`JwtKeys`"))?;
        let claims = credentials(&parts.headers, keys)?;
        let pool = parts
            .extensions
            .get::<PgPool>()
            .ok_or_else(|| wiring("the `PgPool`"))?;
        let account = session_state(pool, claims.sub.into()).await?;
        admit(&claims, account)
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, AppError> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        match user.role {
            Role::Admin => Ok(AdminUser(user)),
            Role::User => Err(AppError::Forbidden("this needs the admin role".to_owned())),
        }
    }
}

/// The verified claims of the bearer token in `headers`.
fn credentials(headers: &HeaderMap, keys: &JwtKeys) -> Result<Claims, AppError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("a bearer token is required".to_owned()))?;
    keys.decode(token)
}

/// The caller of a valid token, given its account's `state` (`None` for no
/// live account).
fn admit(claims: &Claims, state: Option<SessionStateRow>) -> Result<AuthUser, AppError> {
    let revoked = || AppError::Unauthorized("the token is no longer valid".to_owned());
    let state = state.ok_or_else(revoked)?;
    if !state.is_active || state.token_version != claims.tv {
        return Err(revoked());
    }
    Ok(AuthUser {
        id: claims.sub,
        role: state.role,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(tv: i32) -> Claims {
        Claims {
            sub: Uuid::now_v7(),
            role: Role::User,
            tv,
            exp: u64::MAX,
        }
    }

    fn state(token_version: i32, is_active: bool, role: Role) -> SessionStateRow {
        SessionStateRow {
            token_version,
            is_active,
            role,
        }
    }

    #[test]
    fn a_live_account_is_admitted_with_its_role_from_the_database() -> Result<(), AppError> {
        let claims = claims(2);
        let user = admit(&claims, Some(state(2, true, Role::Admin)))?;
        assert_eq!(
            user,
            AuthUser {
                id: claims.sub,
                role: Role::Admin
            }
        );
        Ok(())
    }

    #[test]
    fn a_stale_token_version_is_rejected() {
        let rejected = admit(&claims(1), Some(state(2, true, Role::User)));
        assert!(
            matches!(rejected, Err(AppError::Unauthorized(_))),
            "{rejected:?}"
        );
    }

    #[test]
    fn a_disabled_or_missing_account_is_rejected() {
        let rejected = admit(&claims(1), Some(state(1, false, Role::User)));
        assert!(
            matches!(rejected, Err(AppError::Unauthorized(_))),
            "{rejected:?}"
        );
        let rejected = admit(&claims(1), None);
        assert!(
            matches!(rejected, Err(AppError::Unauthorized(_))),
            "{rejected:?}"
        );
    }
}
