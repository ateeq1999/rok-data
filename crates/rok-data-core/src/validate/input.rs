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
            .filter(|rule| errors.field(&rule.field).next().is_none())
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

/// A field holding input of its own — an [`Input`], or an `Option` or a
/// `Vec` of them — marked `#[rok(nested)]`: its transforms, field rules
/// and built-in database rules run with the outer input's, their errors
/// at `field.inner` (and `items.2.inner` in a list). A nested input's
/// custom database rules run only through its own `validate`.
pub trait Nested: Sized {
    /// The transforms, applied to every inner input.
    #[doc(hidden)]
    fn __transform_nested(self) -> Self;

    /// The field and record rules' failures, under `path`.
    #[doc(hidden)]
    fn __check_nested(&self, path: &str, errors: &mut ValidationErrors);

    /// The built-in database rules, under `path`.
    #[doc(hidden)]
    fn __database_nested(&self, path: &str, rules: &mut Vec<DatabaseRule>);
}

impl<T: Input> Nested for T {
    fn __transform_nested(self) -> Self {
        self.__transform()
    }

    fn __check_nested(&self, path: &str, errors: &mut ValidationErrors) {
        errors.merge(self.__check().prefixed(path));
    }

    fn __database_nested(&self, path: &str, rules: &mut Vec<DatabaseRule>) {
        rules.extend(
            self.__database_rules()
                .into_iter()
                .map(|rule| rule.prefixed(path)),
        );
    }
}

impl<T: Input> Nested for Option<T> {
    fn __transform_nested(self) -> Self {
        self.map(Input::__transform)
    }

    fn __check_nested(&self, path: &str, errors: &mut ValidationErrors) {
        if let Some(inner) = self {
            inner.__check_nested(path, errors);
        }
    }

    fn __database_nested(&self, path: &str, rules: &mut Vec<DatabaseRule>) {
        if let Some(inner) = self {
            inner.__database_nested(path, rules);
        }
    }
}

impl<T: Input> Nested for Vec<T> {
    fn __transform_nested(self) -> Self {
        self.into_iter().map(Input::__transform).collect()
    }

    fn __check_nested(&self, path: &str, errors: &mut ValidationErrors) {
        for (i, inner) in self.iter().enumerate() {
            inner.__check_nested(&format!("{path}.{i}"), errors);
        }
    }

    fn __database_nested(&self, path: &str, rules: &mut Vec<DatabaseRule>) {
        for (i, inner) in self.iter().enumerate() {
            inner.__database_nested(&format!("{path}.{i}"), rules);
        }
    }
}
