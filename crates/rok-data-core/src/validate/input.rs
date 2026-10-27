//! Input: untrusted data on its way to becoming rows, validated before
//! anything is written (workspace ADR 0021).

use crate::exec::Journey;
use crate::{Error, Result};

use super::{DatabaseRule, ValidationErrors};

/// An input struct, made with `#[derive(Input)]`: its fields carry
/// transforms, field rules and database rules, and
/// [`validate`](Input::validate) checks it before it becomes rows.
///
/// ```ignore
/// #[derive(Input)]
/// struct SignUp {
///     #[rok(transform(normalize_email), validate(email, unique(User::EMAIL, case_insensitive)))]
///     email: String,
///     #[rok(validate(exists(Org::ID)))]
///     org_id: i64,
/// }
///
/// let sign_up = body.validate(&db).await?; // transformed, or Error::Validation
/// ```
pub trait Input: Sized + Send + Sync {
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

    /// The built-in database rules (`unique`, `exists`) of every field.
    #[doc(hidden)]
    fn __database_rules(&self) -> Vec<DatabaseRule> {
        Vec::new()
    }

    /// The custom database rules (`database = path`, then
    /// `validate_with_database`), each on `journey`; a field's rule is
    /// skipped when `failed` already holds an error for it.
    #[doc(hidden)]
    async fn __custom_database(
        &self,
        journey: &mut Journey<'_>,
        failed: &ValidationErrors,
    ) -> Result<ValidationErrors> {
        let _ = (journey, failed);
        Ok(ValidationErrors::new())
    }

    /// Transform the input, then check it: field rules (every failure
    /// collected); the built-in database rules of the fields that passed,
    /// all in one statement on `session`; then the custom database rules,
    /// one query each. Returns the transformed input, or
    /// [`Error::Validation`] with every failure.
    ///
    /// The session is any a query takes. Validating inside the write's
    /// transaction narrows the window in which another write can break a
    /// database rule; the table's constraint stays the guarantee.
    async fn validate<'c>(self, session: impl Into<Journey<'c>> + Send) -> Result<Self> {
        let input = self.__transform();
        let mut errors = input.__check();
        let mut journey = session.into();
        let rules: Vec<DatabaseRule> = input
            .__database_rules()
            .into_iter()
            .filter(|rule| errors.field(rule.field).next().is_none())
            .collect();
        errors.merge(super::database::run(&mut journey, rules).await?);
        let custom = input.__custom_database(&mut journey, &errors).await?;
        errors.merge(custom);
        if errors.is_empty() {
            Ok(input)
        } else {
            Err(Error::Validation(errors))
        }
    }

    /// Transforms and field rules only, without a session: for input with
    /// no database rules (or a test). Database rules are not checked.
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
