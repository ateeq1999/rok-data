//! The helpers the generated checks and transforms of string rules call
//! that tie to `validator` and `serde`, written once into
//! `shared/validation.rs` when any feature's rules need them, laid out as
//! rustfmt lays them out. The rule checks themselves (`is_url`,
//! `normalize_email`, …) are the runtime's `rok_data::validate::catalogue`
//! (workspace ADR 0021). Each starts with a newline, the blank line before
//! it.

/// The `check` helper.
pub const CHECK: &str = r#"
/// `Ok` when `valid`, otherwise the error `code`.
pub fn check(valid: bool, code: &'static str) -> Result<(), validator::ValidationError> {
    if valid {
        Ok(())
    } else {
        Err(validator::ValidationError::new(code))
    }
}
"#;

/// The `matches_regex` helper.
pub const MATCHES_REGEX: &str = r#"
/// Whether `value` matches `regex`, compiled from a pattern rok sqlc checked.
#[must_use]
pub fn matches_regex(value: &str, regex: Option<&regex::Regex>) -> bool {
    regex.is_some_and(|regex| regex.is_match(value))
}
"#;

/// The `transformed` helper.
pub const TRANSFORMED: &str = r#"
/// A body field a column's transforms apply to: text, optional text, or
/// nullable text, where `null` is `Some(None)`.
pub trait Transformed<'de>: Sized {
    /// Deserializes the field, transformed by `transform`.
    fn transformed<D>(deserializer: D, transform: fn(&str) -> String) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>;
}

impl<'de> Transformed<'de> for String {
    fn transformed<D>(deserializer: D, transform: fn(&str) -> String) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        serde::Deserialize::deserialize(deserializer).map(|value: String| transform(&value))
    }
}

impl<'de> Transformed<'de> for Option<String> {
    fn transformed<D>(deserializer: D, transform: fn(&str) -> String) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        serde::Deserialize::deserialize(deserializer)
            .map(|value: Option<String>| value.map(|value| transform(&value)))
    }
}

impl<'de> Transformed<'de> for Option<Option<String>> {
    fn transformed<D>(deserializer: D, transform: fn(&str) -> String) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        serde::Deserialize::deserialize(deserializer)
            .map(|value: Option<String>| Some(value.map(|value| transform(&value))))
    }
}
"#;
