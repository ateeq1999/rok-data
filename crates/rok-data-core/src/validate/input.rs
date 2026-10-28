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
    /// What the database rules may read besides the input — the caller, a
    /// tenant — declared with `#[rok(context = Caller)]` and passed to
    /// [`validate_with`](Self::validate_with); `()` when the input declares
    /// none.
    type Context: Sync;

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

    /// The input's schema: its fields' types and the rules a schema can
    /// say.
    #[doc(hidden)]
    fn __schema() -> super::Schema {
        super::Schema::object(Vec::new(), Vec::new())
    }

    /// The input as JSON Schema (2020-12): field types, `required`, and
    /// the field rules a schema can say (`minLength`, `maximum`, `format`,
    /// …). Database rules and custom rules are left out.
    #[cfg(any(feature = "json", feature = "axum"))]
    fn json_schema() -> serde_json::Value {
        let mut schema = Self::__schema().to_json();
        schema["$schema"] = "https://json-schema.org/draft/2020-12/schema".into();
        schema
    }

    /// The built-in database rules (`unique`, `exists`) of every field.
    #[doc(hidden)]
    fn __database_rules(&self, context: &Self::Context) -> Vec<DatabaseRule> {
        let _ = context;
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
        context: &Self::Context,
    ) -> Result<ValidationErrors> {
        let _ = (journey, failed, context);
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
    async fn validate<'c>(self, session: impl Into<Journey<'c>> + Send) -> Result<Self>
    where
        Self: Input<Context = ()>,
    {
        self.validate_with(session, &()).await
    }

    /// [`validate`](Self::validate) with the input's context: the caller
    /// (or tenant) its database rules' `scope`s and custom rules read, so
    /// `exists` can mean "a row this caller can see".
    async fn validate_with<'c>(
        self,
        session: impl Into<Journey<'c>> + Send,
        context: &Self::Context,
    ) -> Result<Self> {
        let input = self.__transform();
        let mut errors = input.__check();
        let mut journey = session.into();
        let rules: Vec<DatabaseRule> = input
            .__database_rules(context)
            .into_iter()
            .filter(|rule| errors.field(&rule.field).next().is_none())
            .collect();
        errors.merge(super::database::run(&mut journey, rules).await?);
        let custom = input
            .__custom_database(&mut journey, &errors, context)
            .await?;
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
/// custom database rules run only through its own `validate`. A nested input
/// shares its parent's context type `C`.
pub trait Nested<C>: Sized {
    /// The transforms, applied to every inner input.
    #[doc(hidden)]
    fn __transform_nested(self) -> Self;

    /// The field and record rules' failures, under `path`.
    #[doc(hidden)]
    fn __check_nested(&self, path: &str, errors: &mut ValidationErrors);

    /// The built-in database rules, under `path`.
    #[doc(hidden)]
    fn __database_nested(&self, path: &str, rules: &mut Vec<DatabaseRule>, context: &C);

    /// The schema of the nested value.
    #[doc(hidden)]
    fn __nested_schema() -> super::Schema;
}

impl<T: Input> Nested<T::Context> for T {
    fn __nested_schema() -> super::Schema {
        T::__schema()
    }

    fn __transform_nested(self) -> Self {
        self.__transform()
    }

    fn __check_nested(&self, path: &str, errors: &mut ValidationErrors) {
        errors.merge(self.__check().prefixed(path));
    }

    fn __database_nested(&self, path: &str, rules: &mut Vec<DatabaseRule>, context: &T::Context) {
        rules.extend(
            self.__database_rules(context)
                .into_iter()
                .map(|rule| rule.prefixed(path)),
        );
    }
}

impl<T: Input> Nested<T::Context> for Option<T> {
    fn __nested_schema() -> super::Schema {
        T::__schema().nullable()
    }

    fn __transform_nested(self) -> Self {
        self.map(Input::__transform)
    }

    fn __check_nested(&self, path: &str, errors: &mut ValidationErrors) {
        if let Some(inner) = self {
            inner.__check_nested(path, errors);
        }
    }

    fn __database_nested(&self, path: &str, rules: &mut Vec<DatabaseRule>, context: &T::Context) {
        if let Some(inner) = self {
            inner.__database_nested(path, rules, context);
        }
    }
}

impl<T: Input> Nested<T::Context> for Vec<T> {
    fn __nested_schema() -> super::Schema {
        super::Schema::array(T::__schema())
    }

    fn __transform_nested(self) -> Self {
        self.into_iter().map(Input::__transform).collect()
    }

    fn __check_nested(&self, path: &str, errors: &mut ValidationErrors) {
        for (i, inner) in self.iter().enumerate() {
            inner.__check_nested(&format!("{path}.{i}"), errors);
        }
    }

    fn __database_nested(&self, path: &str, rules: &mut Vec<DatabaseRule>, context: &T::Context) {
        for (i, inner) in self.iter().enumerate() {
            inner.__database_nested(&format!("{path}.{i}"), rules, context);
        }
    }
}
