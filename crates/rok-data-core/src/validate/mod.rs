//! Validation of model fields, run automatically before `insert`, `update`,
//! `upsert` and `insert_many`.
//!
//! Declare rules on fields with `#[rok(validate(…))]`:
//!
//! ```ignore
//! #[derive(Model)]
//! #[rok(validate_with = check_user)]          // whole-record rule
//! struct User {
//!     id: i64,
//!     #[rok(validate(length(min = 1, max = 50)))]
//!     name: String,
//!     #[rok(validate(email))]
//!     email: String,
//!     #[rok(validate(range(min = 0, max = 150)))]
//!     age: i32,
//!     #[rok(validate(custom = no_spaces))]       // fn(&str) -> Result<(), String>
//!     handle: String,
//! }
//!
//! fn check_user(user: &User) -> Result<(), ValidationErrors> { Ok(()) }
//! ```
//!
//! Rules on `Option` fields only apply when the value is `Some`. Failures
//! are collected (not short-circuited) into [`ValidationErrors`] and
//! returned as [`Error::Validation`](crate::Error::Validation).

use std::fmt;

pub mod catalogue;
mod database;
mod input;
mod messages;
pub mod patterns;
#[cfg(any(feature = "json", feature = "axum"))]
mod report;
pub mod schema;
pub mod transform;

pub use database::DatabaseRule;
pub use input::{Input, Nested};
pub use messages::{Messages, render};
#[cfg(any(feature = "json", feature = "axum"))]
pub use report::{DetailsReporter, ErrorReporter};
pub use schema::{Probe, Schema, SchemaType};

/// Whether a value counts as given, for `required_if` / `required_unless` /
/// `required_with`: `Some`, non-empty text, a non-empty list.
pub trait Presence {
    /// `true` when the value is given.
    fn is_present(&self) -> bool;
}

impl<T> Presence for Option<T> {
    fn is_present(&self) -> bool {
        self.is_some()
    }
}

impl Presence for String {
    fn is_present(&self) -> bool {
        !self.is_empty()
    }
}

impl Presence for &str {
    fn is_present(&self) -> bool {
        !self.is_empty()
    }
}

impl<T> Presence for Vec<T> {
    fn is_present(&self) -> bool {
        !self.is_empty()
    }
}

/// One failed rule.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldError {
    /// Where the failing value is: the field's name, or a dotted path into
    /// nested input (`address.city`, `items.2.sku`); `""` for record-level
    /// errors.
    pub field: String,
    /// Machine-readable rule name: `length`, `range`, `email`,
    /// `non_empty`, `custom`, …
    pub code: &'static str,
    /// Human-readable message.
    pub message: String,
    /// The rule's arguments (`min`, `max`, …), which messages quote as
    /// `{min}` and the reporter answers as `params`.
    pub params: Vec<(&'static str, crate::Value)>,
    /// Whether the message is the rule's own (`message = "…"`), which a
    /// [`Messages`] provider leaves as it is.
    custom: bool,
}

impl FieldError {
    /// Whether the message came from the rule's `message = "…"`.
    pub fn has_custom_message(&self) -> bool {
        self.custom
    }
}

/// Every validation failure of a record.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ValidationErrors {
    errors: Vec<FieldError>,
}

impl ValidationErrors {
    /// No errors yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a failure.
    pub fn add(
        &mut self,
        field: impl Into<String>,
        code: &'static str,
        message: impl Into<String>,
    ) {
        self.errors.push(FieldError {
            field: field.into(),
            code,
            message: message.into(),
            params: Vec::new(),
            custom: false,
        });
    }

    /// Append every error of `other`.
    pub fn merge(&mut self, other: ValidationErrors) {
        self.errors.extend(other.errors);
    }

    /// `true` if nothing failed.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// All failures, in declaration order.
    pub fn errors(&self) -> &[FieldError] {
        &self.errors
    }

    /// Failures of one field.
    pub fn field(&self, field: &str) -> impl Iterator<Item = &FieldError> {
        self.errors.iter().filter(move |e| e.field == field)
    }

    /// Every error moved under `prefix`: `city` becomes `address.city`, a
    /// record-level error becomes `address`'s own.
    #[must_use]
    pub fn prefixed(mut self, prefix: &str) -> Self {
        for error in &mut self.errors {
            error.field = if error.field.is_empty() {
                prefix.to_owned()
            } else {
                format!("{prefix}.{}", error.field)
            };
        }
        self
    }

    /// `Ok(())` if empty, `Err(self)` otherwise.
    pub fn into_result(self) -> Result<(), ValidationErrors> {
        if self.is_empty() { Ok(()) } else { Err(self) }
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.errors.iter().enumerate() {
            if i > 0 {
                f.write_str("; ")?;
            }
            if e.field.is_empty() {
                f.write_str(&e.message)?;
            } else {
                write!(f, "{}: {}", e.field, e.message)?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for ValidationErrors {}

/// Values with a length; `None` means "absent, skip the rule".
pub trait HasLength {
    /// Length in characters (strings) or elements (collections).
    fn length(&self) -> Option<usize>;
}

impl HasLength for str {
    fn length(&self) -> Option<usize> {
        Some(self.chars().count())
    }
}

impl HasLength for String {
    fn length(&self) -> Option<usize> {
        self.as_str().length()
    }
}

impl<T> HasLength for Vec<T> {
    fn length(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<T: HasLength + ?Sized> HasLength for &T {
    fn length(&self) -> Option<usize> {
        (**self).length()
    }
}

impl<T: HasLength> HasLength for Option<T> {
    fn length(&self) -> Option<usize> {
        self.as_ref().and_then(HasLength::length)
    }
}

/// Numeric values; `None` means "absent, skip the rule".
pub trait AsNumber {
    /// The value as `f64`.
    fn as_number(&self) -> Option<f64>;
}

macro_rules! as_number {
    ($($t:ty),*) => {$(
        impl AsNumber for $t {
            fn as_number(&self) -> Option<f64> {
                Some(*self as f64)
            }
        }
    )*};
}

as_number!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);

impl<T: AsNumber> AsNumber for Option<T> {
    fn as_number(&self) -> Option<f64> {
        self.as_ref().and_then(AsNumber::as_number)
    }
}

/// Text values; `None` means "absent, skip the rule".
pub trait AsText {
    /// The value as `&str`.
    fn as_text(&self) -> Option<&str>;
}

impl AsText for String {
    fn as_text(&self) -> Option<&str> {
        Some(self)
    }
}

impl AsText for str {
    fn as_text(&self) -> Option<&str> {
        Some(self)
    }
}

impl<T: AsText> AsText for Option<T> {
    fn as_text(&self) -> Option<&str> {
        self.as_ref().and_then(AsText::as_text)
    }
}

/// `length(min, max)` rule.
pub fn length<T: HasLength + ?Sized>(
    value: &T,
    min: Option<usize>,
    max: Option<usize>,
) -> Result<(), String> {
    let Some(len) = value.length() else {
        return Ok(());
    };
    match (min, max) {
        (Some(min), _) if len < min => Err(match max {
            Some(max) => format!("length must be between {min} and {max}"),
            None => format!("length must be at least {min}"),
        }),
        (_, Some(max)) if len > max => Err(match min {
            Some(min) => format!("length must be between {min} and {max}"),
            None => format!("length must be at most {max}"),
        }),
        _ => Ok(()),
    }
}

/// `non_empty` rule (`length(min = 1)` with a clearer message).
pub fn non_empty<T: HasLength + ?Sized>(value: &T) -> Result<(), String> {
    match value.length() {
        Some(0) => Err("must not be empty".into()),
        _ => Ok(()),
    }
}

/// `range(min, max)` rule (inclusive).
pub fn range<T: AsNumber + ?Sized>(
    value: &T,
    min: Option<f64>,
    max: Option<f64>,
) -> Result<(), String> {
    let Some(n) = value.as_number() else {
        return Ok(());
    };
    let fmt = |x: f64| {
        if x.fract() == 0.0 {
            format!("{x:.0}")
        } else {
            x.to_string()
        }
    };
    match (min, max) {
        (Some(lo), Some(hi)) if n < lo || n > hi => {
            Err(format!("must be between {} and {}", fmt(lo), fmt(hi)))
        }
        (Some(lo), None) if n < lo => Err(format!("must be at least {}", fmt(lo))),
        (None, Some(hi)) if n > hi => Err(format!("must be at most {}", fmt(hi))),
        _ => Ok(()),
    }
}

/// `email` rule: a pragmatic syntax check (`local@domain.tld`, no
/// whitespace), not full RFC 5322.
pub fn email<T: AsText + ?Sized>(value: &T) -> Result<(), String> {
    let Some(s) = value.as_text() else {
        return Ok(());
    };
    let valid = match s.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty()
                && !domain.contains('@')
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !s.chars().any(char::is_whitespace)
        }
        None => false,
    };
    if valid {
        Ok(())
    } else {
        Err("must be a valid email address".into())
    }
}

/// `text` as a `&'static str`: each distinct text is kept once for the
/// process. For the codes and parameter names that arrive at run time
/// (read back from a body, converted from another validator) — a small,
/// fixed vocabulary.
#[doc(hidden)]
pub fn intern(text: &str) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static TEXTS: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut texts = TEXTS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(known) = texts.get(text) {
        return known;
    }
    let kept: &'static str = Box::leak(text.to_owned().into_boxed_str());
    texts.insert(kept);
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules() {
        assert!(length("abc", Some(1), Some(3)).is_ok());
        assert_eq!(
            length("", Some(1), Some(3)).unwrap_err(),
            "length must be between 1 and 3"
        );
        assert_eq!(
            length(&vec![1, 2], None, Some(1)).unwrap_err(),
            "length must be at most 1"
        );
        assert!(
            length(&None::<String>, Some(5), None).is_ok(),
            "None is skipped"
        );
        assert_eq!(length("héllo", None, Some(5)), Ok(()), "counts characters");
        assert!(non_empty("").is_err() && non_empty("x").is_ok());
        assert!(range(&5, Some(0.0), Some(10.0)).is_ok());
        assert_eq!(
            range(&-1, Some(0.0), None).unwrap_err(),
            "must be at least 0"
        );
        assert_eq!(
            range(&2.5_f64, None, Some(2.0)).unwrap_err(),
            "must be at most 2"
        );
        assert!(range(&Some(11), Some(0.0), Some(10.0)).is_err());
        for ok in ["a@b.co", "first.last+tag@sub.example.org"] {
            assert!(email(ok).is_ok(), "{ok}");
        }
        for bad in [
            "", "a", "a@b", "@b.co", "a@.co", "a@b.", "a b@c.de", "a@b@c.de",
        ] {
            assert!(email(bad).is_err(), "{bad}");
        }
        assert!(email(&None::<String>).is_ok());
    }

    #[test]
    fn errors_collect_and_display() {
        let mut errors = ValidationErrors::new();
        errors.add("name", "length", "too long");
        errors.add("", "custom", "dates are inverted");
        assert_eq!(errors.field("name").count(), 1);
        assert_eq!(errors.to_string(), "name: too long; dates are inverted");
        assert!(errors.into_result().is_err());
        assert!(ValidationErrors::new().into_result().is_ok());
    }
}
