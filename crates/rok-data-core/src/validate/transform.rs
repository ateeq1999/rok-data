//! Transforms an input's fields go through before their rules:
//! `#[rok(transform(trim, lowercase))]`. Each takes the field's value and
//! returns the new one, so `Option` fields (a patch's `Option<Option<_>>`
//! too) keep `None`.

/// A field a built-in transform can apply to: text, or optional text.
pub trait Transformable: Sized {
    /// The value with `f` applied to its text (`None` stays `None`).
    fn map_text(self, f: impl FnOnce(String) -> String) -> Self;
}

impl Transformable for String {
    fn map_text(self, f: impl FnOnce(String) -> String) -> Self {
        f(self)
    }
}

impl<T: Transformable> Transformable for Option<T> {
    fn map_text(self, f: impl FnOnce(String) -> String) -> Self {
        self.map(|value| value.map_text(f))
    }
}

/// Leading and trailing whitespace removed.
pub fn trim<T: Transformable>(value: T) -> T {
    value.map_text(|s| s.trim().to_owned())
}

/// Lowercased.
pub fn lowercase<T: Transformable>(value: T) -> T {
    value.map_text(|s| s.to_lowercase())
}

/// Uppercased.
pub fn uppercase<T: Transformable>(value: T) -> T {
    value.map_text(|s| s.to_uppercase())
}

/// An email address normalized as the generated APIs' `normalizeEmail`
/// does it ([`catalogue::normalize_email`](super::catalogue::normalize_email)):
/// the domain and local part lowercased, a provider's subaddress and
/// Gmail's single dots removed.
pub fn normalize_email<T: Transformable>(value: T) -> T {
    value.map_text(|s| super::catalogue::normalize_email(&s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms_apply_to_text_and_keep_none() {
        assert_eq!(trim("  a b ".to_owned()), "a b");
        assert_eq!(lowercase(Some("AbC".to_owned())), Some("abc".to_owned()));
        assert_eq!(uppercase(None::<String>), None);
        assert_eq!(
            normalize_email("A.nn+news@GoogleMail.com".to_owned()),
            "ann@gmail.com"
        );
    }
}
