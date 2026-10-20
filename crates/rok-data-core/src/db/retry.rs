//! Whether an error is worth retrying.

use crate::Error;

/// Errors that can tell whether retrying the transaction might succeed.
/// Implemented for [`Error`]; implement it for your own error type (usually
/// by delegating to a wrapped `rok_data::Error`) to use it with
/// [`Db::transaction_with`](crate::Db::transaction_with).
pub trait Retryable {
    /// `true` for serialization failures and deadlocks.
    fn is_retryable(&self) -> bool;
}

impl Retryable for Error {
    fn is_retryable(&self) -> bool {
        self.is_serialization_failure()
    }
}
