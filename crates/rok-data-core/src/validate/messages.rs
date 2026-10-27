//! Error messages: a rule's own (`message = "…"`), an app-wide
//! [`Messages`] provider (translations), or the built-in English one — in
//! that order. Templates quote `{field}` and the rule's arguments
//! (`{min}`, `{max}`, …).

use crate::Value;

use super::{FieldError, ValidationErrors};

/// `template` with `{field}` and each `{param}` filled in.
pub fn render(template: &str, field: &str, params: &[(&'static str, Value)]) -> String {
    let mut message = template.replace("{field}", field);
    for (name, value) in params {
        let shown = match value {
            Value::String(Some(text)) => text.clone(),
            other => other.to_string(),
        };
        message = message.replace(&format!("{{{name}}}"), &shown);
    }
    message
}

/// The messages of an app, keyed by rule code and optionally by field:
/// translations, or a product's own wording. A template may quote
/// `{field}` and the rule's arguments.
///
/// ```ignore
/// struct Norwegian;
/// impl Messages for Norwegian {
///     fn template(&self, code: &str, _field: &str) -> Option<String> {
///         match code {
///             "required" => Some("{field} må fylles ut".into()),
///             "length" => Some("{field} må ha minst {min} tegn".into()),
///             _ => None,
///         }
///     }
/// }
/// let errors = errors.localized(&Norwegian);
/// ```
pub trait Messages {
    /// The template for a failure of rule `code` on `field` (a path), or
    /// `None` to keep the built-in message.
    fn template(&self, code: &str, field: &str) -> Option<String>;
}

impl ValidationErrors {
    /// Record a failed rule with its arguments: the message is
    /// `template` rendered when the rule has its own (`message = "…"`),
    /// `default` otherwise.
    #[doc(hidden)]
    pub fn add_rule(
        &mut self,
        field: impl Into<String>,
        code: &'static str,
        default: impl Into<String>,
        template: Option<&str>,
        params: Vec<(&'static str, Value)>,
    ) {
        let field = field.into();
        let (message, custom) = match template {
            Some(template) => (render(template, &field, &params), true),
            None => (default.into(), false),
        };
        self.errors.push(FieldError {
            field,
            code,
            message,
            params,
            custom,
        });
    }

    /// The errors with `messages`' templates in place of the built-in
    /// messages; a rule's own message (`message = "…"`) stays.
    #[must_use]
    pub fn localized(mut self, messages: &impl Messages) -> Self {
        for error in &mut self.errors {
            if error.custom {
                continue;
            }
            if let Some(template) = messages.template(error.code, &error.field) {
                error.message = render(&template, &error.field, &error.params);
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Terse;

    impl Messages for Terse {
        fn template(&self, code: &str, _field: &str) -> Option<String> {
            (code == "length").then(|| "{field}: {min}+".to_owned())
        }
    }

    #[test]
    fn a_rules_own_message_beats_the_provider_which_beats_the_default() {
        let mut errors = ValidationErrors::new();
        let min = vec![("min", Value::from(3_i64))];
        errors.add_rule("name", "length", "name is too short", None, min.clone());
        errors.add_rule(
            "nick",
            "length",
            "nick is too short",
            Some("at least {min} characters, {field}"),
            min,
        );
        errors.add_rule("email", "email", "email is not valid", None, Vec::new());
        let messages: Vec<String> = errors
            .localized(&Terse)
            .errors()
            .iter()
            .map(|e| e.message.clone())
            .collect();
        assert_eq!(
            messages,
            [
                "name: 3+",
                "at least 3 characters, nick",
                "email is not valid"
            ]
        );
    }
}
