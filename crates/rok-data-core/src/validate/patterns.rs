//! The pattern rules: `mobile` (by locale), `postal_code` (by country) and
//! `regex`, the generated APIs' `mobile`, `postalCode` and `regex`. The
//! tables are validator.js's patterns (`\d` written `[0-9]`, `/i` as
//! `(?i)`), the same the engine generates into an API's checks — a test in
//! `rok-cli` keeps the two copies equal. Checking needs the `regex`
//! feature.

/// The mobile phone patterns, by locale.
pub const MOBILE_PATTERNS: &[(&str, &str)] = &[
    ("ar-AE", r"^((\+?971)|0)?5[024568][0-9]{7}$"),
    ("ar-BH", r"^(\+?973)?(3|6)[0-9]{7}$"),
    ("ar-EG", r"^((\+?20)|0)?1[0125][0-9]{8}$"),
    ("ar-JO", r"^(\+?962|0)?7[789][0-9]{7}$"),
    ("ar-KW", r"^(\+?965)([569][0-9]{7}|41[0-9]{6})$"),
    ("ar-OM", r"^((\+|00)968)?([79][1-9])[0-9]{6}$"),
    ("ar-QA", r"^(\+?974|0)?([3567][0-9]{7})$"),
    ("ar-SA", r"^(!?(\+?966)|0)?5[0-9]{8}$"),
    (
        "de-DE",
        r"^((\+49|0)1)(5[0-25-9][0-9]|6([23]|0[0-9]?)|7([0-57-9]|6[0-9]))[0-9]{7,9}$",
    ),
    (
        "en-CA",
        r"^((\+1|1)?( |-)?)?(\([2-9][0-9]{2}\)|[2-9][0-9]{2})( |-)?([2-9][0-9]{2}( |-)?[0-9]{4})$",
    ),
    (
        "en-US",
        r"^((\+1|1)?( |-)?)?(\([2-9][0-9]{2}\)|[2-9][0-9]{2})( |-)?([2-9][0-9]{2}( |-)?[0-9]{4})$",
    ),
    ("en-GB", r"^(\+?44|0)7[1-9][0-9]{8}$"),
    ("en-IN", r"^(\+?91|0)?[6789][0-9]{9}$"),
    ("es-ES", r"^(\+?34)?[6|7][0-9]{8}$"),
    ("fr-FR", r"^(\+?33|0)[67][0-9]{8}$"),
    ("tr-TR", r"^(\+?90|0)?5[0-9]{9}$"),
];

/// The postal code patterns, by country.
pub const POSTAL_PATTERNS: &[(&str, &str)] = &[
    (
        "CA",
        r"(?i)^[ABCEGHJKLMNPRSTVXY][0-9][ABCEGHJ-NPRSTV-Z][\s\-]?[0-9][ABCEGHJ-NPRSTV-Z][0-9]$",
    ),
    ("DE", r"^[0-9]{5}$"),
    ("SA", r"^[0-9]{5}$"),
    ("ES", r"^(5[0-2]{1}|[0-4]{1}[0-9]{1})[0-9]{3}$"),
    (
        "FR",
        r"^(?:(?:0[1-9]|[1-8][0-9]|9[0-5])[0-9]{3}|97[1-46][0-9]{2})$",
    ),
    (
        "GB",
        r"(?i)^(gir\s?0aa|[a-z]{1,2}[0-9][0-9a-z]?\s?([0-9][a-z]{2})?)$",
    ),
    ("US", r"^[0-9]{5}(-[0-9]{4})?$"),
];

/// The pattern of `key` in `table`.
pub fn pattern_of(table: &[(&str, &'static str)], key: &str) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, p)| *p)
}

#[cfg(feature = "regex")]
mod checks {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    use regex::Regex;

    use super::{MOBILE_PATTERNS, POSTAL_PATTERNS, pattern_of};

    /// `pattern` compiled once per process; `None` when it doesn't compile.
    fn compiled(pattern: &str) -> Option<Regex> {
        static CACHE: OnceLock<Mutex<HashMap<String, Option<Regex>>>> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let mut cache = cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache
            .entry(pattern.to_owned())
            .or_insert_with(|| Regex::new(pattern).ok())
            .clone()
    }

    /// The `regex` rule: `value` matches `pattern` (a pattern that doesn't
    /// compile matches nothing).
    #[must_use]
    pub fn matches_pattern(value: &str, pattern: &str) -> bool {
        compiled(pattern).is_some_and(|regex| regex.is_match(value))
    }

    /// Whether `value` matches the pattern of any of `keys` in `table`.
    fn any_of(value: &str, table: &[(&str, &'static str)], keys: &[&str]) -> bool {
        keys.iter()
            .filter_map(|key| pattern_of(table, key))
            .any(|pattern| matches_pattern(value, pattern))
    }

    /// The `mobile` rule: a mobile number of one of `locales` (any locale
    /// carried, when none); with `strict`, it must start with `+`.
    #[must_use]
    pub fn is_mobile(value: &str, locales: &[&str], strict: bool) -> bool {
        if strict && !value.starts_with('+') {
            return false;
        }
        if locales.is_empty() {
            let all: Vec<&str> = MOBILE_PATTERNS.iter().map(|(locale, _)| *locale).collect();
            any_of(value, MOBILE_PATTERNS, &all)
        } else {
            any_of(value, MOBILE_PATTERNS, locales)
        }
    }

    /// The `postal_code` rule: a postal code of one of `countries`.
    #[must_use]
    pub fn is_postal_code(value: &str, countries: &[&str]) -> bool {
        any_of(value, POSTAL_PATTERNS, countries)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_pattern_rules_check_their_tables() {
            assert!(is_mobile("+966512345678", &["ar-SA"], true));
            assert!(!is_mobile("966512345678", &["ar-SA"], true));
            assert!(is_mobile("07123456789", &[], false));
            assert!(is_postal_code("SW1A 1AA", &["GB"]));
            assert!(!is_postal_code("123", &["US", "DE"]));
            assert!(matches_pattern("ab-12", r"^[a-z]+-[0-9]+$"));
            assert!(
                !matches_pattern("x", "("),
                "a broken pattern matches nothing"
            );
        }
    }
}

#[cfg(feature = "regex")]
pub use checks::{is_mobile, is_postal_code, matches_pattern};
