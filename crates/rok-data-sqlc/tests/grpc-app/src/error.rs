//! [`AppError`]: every error a service returns. `shared/grpc.rs` answers
//! each with its gRPC status. A new error case is a variant here, never an
//! error type in a feature.

use rok_data::ValidationErrors;
use sqlx::error::ErrorKind;

/// An error a service returns. The message of each variant but `Internal`
/// is sent to the caller.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// A malformed request.
    #[error("{0}")]
    BadRequest(String),
    /// No valid credentials.
    #[error("{0}")]
    Unauthorized(String),
    /// Not allowed.
    #[error("{0}")]
    Forbidden(String),
    /// No such row, or another caller's.
    #[error("{0}")]
    NotFound(String),
    /// A unique or foreign-key violation, naming the constraint.
    #[error("{0}")]
    Conflict(String),
    /// A body over the limit.
    #[error("{0}")]
    PayloadTooLarge(String),
    /// Broken rules, with their details by field path when there are any;
    /// also a check violation.
    #[error("{message}")]
    Validation {
        /// What is wrong.
        message: String,
        /// The broken rules by field.
        details: Option<ValidationErrors>,
    },
    /// The version a write needs is missing.
    #[error("{0}")]
    PreconditionRequired(String),
    /// A stale version.
    #[error("{0}")]
    VersionConflict(String),
    /// Anything else. Logged when answered; the caller gets a generic
    /// message.
    #[error("internal error: {0}")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl AppError {
    /// The error's `snake_case` code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            AppError::BadRequest(_) => "bad_request",
            AppError::Unauthorized(_) => "unauthorized",
            AppError::Forbidden(_) => "forbidden",
            AppError::NotFound(_) => "not_found",
            AppError::Conflict(_) => "conflict",
            AppError::PayloadTooLarge(_) => "payload_too_large",
            AppError::Validation { .. } => "validation_failed",
            AppError::PreconditionRequired(_) => "precondition_required",
            AppError::VersionConflict(_) => "version_conflict",
            AppError::Internal(_) => "internal_error",
        }
    }
}

/// Broken validation rules, as `input.validate(&db)` reports them.
impl From<ValidationErrors> for AppError {
    fn from(details: ValidationErrors) -> Self {
        AppError::Validation {
            message: "the request failed validation".to_owned(),
            details: Some(details),
        }
    }
}

/// A unique or foreign-key violation is a `Conflict` naming the constraint,
/// a check violation is a `Validation`, `RowNotFound` is a `NotFound`, and
/// anything else is `Internal`.
impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::RowNotFound = error {
            return AppError::NotFound("no such row".to_owned());
        }
        if let sqlx::Error::Database(database) = &error {
            let constraint = database.constraint().unwrap_or("unnamed");
            match database.kind() {
                ErrorKind::UniqueViolation => {
                    return AppError::Conflict(format!(
                        "the value is already taken (constraint `{constraint}`)"
                    ));
                }
                ErrorKind::ForeignKeyViolation => {
                    return AppError::Conflict(format!(
                        "the change breaks a reference (constraint `{constraint}`)"
                    ));
                }
                ErrorKind::CheckViolation => {
                    return AppError::Validation {
                        message: format!("the value breaks the check `{constraint}`"),
                        details: None,
                    };
                }
                _ => {}
            }
        }
        AppError::Internal(Box::new(error))
    }
}

/// The generated data layer's error: rows map into `NotFound`, concurrent
/// writes into `VersionConflict`, and everything else reuses the
/// `sqlx::Error` mapping where a driver error wrapped it.
impl From<rok_data::Error> for AppError {
    fn from(error: rok_data::Error) -> Self {
        match error {
            rok_data::Error::NotFound { .. } => AppError::NotFound("no such row".to_owned()),
            rok_data::Error::Conflict { .. } => {
                AppError::VersionConflict("the record changed before the write".to_owned())
            }
            rok_data::Error::Validation(details) => AppError::from(details),
            rok_data::Error::Database(sqlx_error) => AppError::from(sqlx_error),
            other => AppError::Internal(Box::new(other)),
        }
    }
}
