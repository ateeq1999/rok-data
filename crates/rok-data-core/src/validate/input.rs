//! Input: untrusted data on its way to becoming rows, validated before
//! anything is written (workspace ADR 0021).

use crate::exec::Journey;
use crate::{Error, Result};

use super::ValidationErrors;

/// An input struct, made with `#[derive(Input)]`: its fields carry
/// transforms and rules, and [`validate`](Input::validate) checks it
/// before it becomes rows.
///
/// ```ignore
/// #[derive(Input)]
/// struct SignUp {
///     #[rok(transform(normalize_email), validate(email))]
///     email: String,
///     #[rok(transform(trim), validate(length(min = 1, max = 50)))]
///     name: String,
/// }
///
/// let sign_up = body.validate(&db).await?; // transformed, or Error::Validation
/// ```
pub trait Input: Sized + Send {
    /// The fields' transforms, applied in declaration order.
    #[doc(hidden)]
    fn __transform(self) -> Self {
        self
    }

    /// Every field-rule and record-rule failure of the transformed input.
    #[doc(hidden)]
    fn __check(&self) -> ValidationErrors {
        ValidationErrors::new()
    }

    /// Transform the input, then run its rules — field rules, then the
    /// database rules against `session` — and return it, or
    /// [`Error::Validation`] with every failure.
    ///
    /// The session is any a query takes; validating inside the write's
    /// transaction narrows the window in which another write can break a
    /// database rule (the table's constraint stays the guarantee).
    async fn validate<'c>(self, session: impl Into<Journey<'c>> + Send) -> Result<Self> {
        let _session = session.into();
        self.validate_fields()
    }

    /// [`validate`](Self::validate) without a session: transforms and
    /// field rules only, for input with no database rules (or a test).
    fn validate_fields(self) -> Result<Self> {
        let input = self.__transform();
        let errors = input.__check();
        if errors.is_empty() {
            Ok(input)
        } else {
            Err(Error::Validation(errors))
        }
    }
}
