//! Who is calling: the [`AuthUser`] an interceptor puts in a request's
//! extensions once it checked the credentials, and the [`AdminUser`] the
//! admin-only services take. Checking a token is the interceptor's.

use uuid::Uuid;

/// A caller's role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Any signed-in caller.
    User,
    /// An administrator.
    Admin,
}

/// A caller with valid credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthUser {
    /// The caller's id.
    pub id: Uuid,
    /// The caller's role.
    pub role: Role,
}

/// A caller with valid credentials and the `admin` role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminUser(pub AuthUser);
