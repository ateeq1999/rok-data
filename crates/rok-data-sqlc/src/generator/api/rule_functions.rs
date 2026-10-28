//! The functions string rules generate into `dto.rs`, each column's checks
//! (`custom` rules) and transforms, and the helpers they call, which
//! `shared/validation.rs` holds once for every feature
//! ([`super::rule_snippets`]).

use std::collections::BTreeSet;

use super::dto::body_columns;
use super::feature::Feature;
use super::layout::{Uses, push_call, push_signature, push_signature_where};
use super::rule_snippets as snippets;
use super::rules::function_name;
use crate::api_rules::Rule;
use crate::generator::sql::str_literal;
use crate::generator::{MAX_WIDTH, doc_name, push_doc};
use crate::schema::Column;

/// rustfmt's default `array_width`.
const ARRAY_WIDTH: usize = 60;

/// rustfmt's default `short_array_element_width_threshold`.
const SHORT_ELEMENT: usize = 10;

/// A helper the rendered functions call, in `shared/validation.rs`, in the
/// order that file holds them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Helper {
    Check,
    MatchesRegex,
    Letters,
    Uuid,
    Ulid,
    HexCode,
    Jwt,
    Iban,
    Coordinates,
    CreditCard,
    Url,
    NormalizeEmail,
    Transformed,
}

impl Helper {
    /// The name a `dto.rs` imports it by.
    fn name(self) -> &'static str {
        match self {
            Helper::Check => "check",
            Helper::MatchesRegex => "matches_regex",
            Helper::Letters => "letters",
            Helper::Uuid => "is_uuid",
            Helper::Ulid => "is_ulid",
            Helper::HexCode => "is_hex_code",
            Helper::Jwt => "is_jwt",
            Helper::Iban => "is_iban",
            Helper::Coordinates => "is_coordinates",
            Helper::CreditCard => "is_credit_card",
            Helper::Url => "is_url",
            Helper::NormalizeEmail => "normalize_email",
            Helper::Transformed => "Transformed",
        }
    }

    /// Whether the runtime's rule catalogue holds it
    /// (`rok_data::validate::catalogue`, workspace ADR 0021); the others are
    /// written into `shared/validation.rs`.
    fn in_runtime(self) -> bool {
        !matches!(
            self,
            Helper::Check | Helper::MatchesRegex | Helper::Transformed
        )
    }

    /// The path a `dto.rs` imports it from.
    fn path(self) -> String {
        if self.in_runtime() {
            format!("rok_data::validate::catalogue::{}", self.name())
        } else {
            format!("crate::shared::validation::{}", self.name())
        }
    }

    /// Its snippets for `shared/validation.rs`; none for a runtime one.
    fn snippets(self) -> &'static [&'static str] {
        match self {
            Helper::Check => &[snippets::CHECK],
            Helper::MatchesRegex => &[snippets::MATCHES_REGEX],
            Helper::Transformed => &[snippets::TRANSFORMED],
            _ => &[],
        }
    }
}

/// The helpers some rendered functions call.
type Needs = BTreeSet<Helper>;

/// The helpers `features`' string rules call, for `shared/validation.rs`:
/// each needed one once, each snippet with the blank line before it.
pub fn shared_helpers(features: &[Feature<'_>]) -> String {
    let mut needs = Needs::new();
    for feature in features {
        needs.extend(render_functions(&mut String::new(), feature));
    }
    needs
        .into_iter()
        .flat_map(|helper| helper.snippets().iter().copied())
        .collect()
}

/// Appends the functions checking and transforming the fields of the
/// create and update bodies, each column once, and imports the helpers they
/// call from `shared/validation.rs`.
pub fn push_functions(output: &mut String, uses: &mut Uses, feature: &Feature<'_>) {
    for helper in render_functions(output, feature) {
        uses.add(&helper.path());
    }
}

/// The functions of [`push_functions`], returning the helpers they call.
fn render_functions(output: &mut String, feature: &Feature<'_>) -> Needs {
    let mut done: Vec<&str> = Vec::new();
    let mut needs = Needs::new();
    for column in body_columns(feature) {
        let rules = feature.rules_of(column);
        if rules.is_empty() || done.contains(&column.sql_name.as_str()) {
            continue;
        }
        done.push(&column.sql_name);
        if rules.iter().any(Rule::is_transform) {
            needs.insert(Helper::Transformed);
            push_transforms(output, column, rules, &mut needs);
        }
        for rule in rules.iter().filter(|rule| !rule.is_transform()) {
            if !matches!(rule, Rule::Email | Rule::IpAddress(_)) {
                needs.insert(Helper::Check);
                push_check_function(output, column, rule, &mut needs);
            }
        }
    }
    needs
}

/// `{column}_transforms`, applying the column's transforms in order, and
/// `{column}_transformed`, the body field's `deserialize_with`.
fn push_transforms(output: &mut String, column: &Column, rules: &[Rule], needs: &mut Needs) {
    let transforms: Vec<&Rule> = rules.iter().filter(|rule| rule.is_transform()).collect();
    let names: Vec<String> = transforms
        .iter()
        .map(|rule| format!("`{}`", rule.name()))
        .collect();
    let doc_column = doc_name(&column.sql_name);
    output.push('\n');
    push_doc(
        output,
        0,
        &[&format!(
            "The {doc_column} column's transforms: {}.",
            names.join(", then ")
        )],
    );
    let name = format!("{}_transforms", column.rust_name);
    let head = format!("fn {name}");
    push_signature(output, 0, &head, &["value: &str".to_owned()], "String");
    // `value` is a `&str` until a transform makes a `String`.
    let mut borrowed = true;
    for (index, rule) in transforms.iter().enumerate() {
        let expr = match rule {
            Rule::Trim => "value.trim()".to_owned(),
            Rule::ToLowerCase => "value.to_lowercase()".to_owned(),
            Rule::ToUpperCase => "value.to_uppercase()".to_owned(),
            _ => {
                needs.insert(Helper::NormalizeEmail);
                if borrowed {
                    "normalize_email(value)".to_owned()
                } else {
                    "normalize_email(&value)".to_owned()
                }
            }
        };
        borrowed = matches!(rule, Rule::Trim);
        if index + 1 < transforms.len() {
            output.push_str(&format!("    let value = {expr};\n"));
        } else if borrowed {
            output.push_str(&format!("    {expr}.to_owned()\n"));
        } else {
            output.push_str(&format!("    {expr}\n"));
        }
    }
    output.push_str("}\n\n");
    push_doc(
        output,
        0,
        &[&format!(
            "Deserializes the {doc_column} column, transformed."
        )],
    );
    let head = format!("fn {}_transformed<'de, D, T>", column.rust_name);
    push_signature_where(
        output,
        0,
        &head,
        &["deserializer: D".to_owned()],
        "Result<T, D::Error>",
        &["D: serde::Deserializer<'de>", "T: Transformed<'de>"],
    );
    let args = ["deserializer".to_owned(), name];
    push_call(output, 4, "", "T::transformed", &args, "");
    output.push_str("}\n");
}

/// A `const {name}: &[&str]` at `indent`, laid out as rustfmt lays out an
/// array: on one line when it fits, otherwise short elements packed on each
/// line, longer ones one per line.
fn push_str_array(output: &mut String, indent: usize, name: &str, values: &[String]) {
    let pad = " ".repeat(indent);
    let items: Vec<String> = values.iter().map(|value| str_literal(value)).collect();
    let array = format!("[{}]", items.join(", "));
    let one_line = format!("{pad}const {name}: &[&str] = &{array};");
    if array.len() <= ARRAY_WIDTH && one_line.len() <= MAX_WIDTH {
        output.push_str(&one_line);
        output.push('\n');
        return;
    }
    output.push_str(&format!("{pad}const {name}: &[&str] = &[\n"));
    let inner = format!("{pad}    ");
    if items.iter().all(|item| item.len() <= SHORT_ELEMENT) {
        let mut line = String::new();
        for item in &items {
            if !line.is_empty() && inner.len() + line.len() + 1 + item.len() + 1 > MAX_WIDTH {
                output.push_str(&format!("{inner}{}\n", line.trim_end()));
                line.clear();
            }
            line.push_str(&format!("{item}, "));
        }
        output.push_str(&format!("{inner}{}\n", line.trim_end()));
    } else {
        for item in &items {
            output.push_str(&format!("{inner}{item},\n"));
        }
    }
    output.push_str(&format!("{pad}];\n"));
}

/// The regular expression of a `regex`, `mobile` or `postalCode` rule.
fn pattern(rule: &Rule) -> Option<String> {
    let alternatives = |patterns: Vec<&str>| {
        if let [one] = patterns.as_slice() {
            (*one).to_owned()
        } else {
            let groups: Vec<String> = patterns.iter().map(|p| format!("(?:{p})")).collect();
            groups.join("|")
        }
    };
    match rule {
        Rule::Regex(pattern) => Some(pattern.clone()),
        Rule::Mobile { locales, .. } => {
            // No locale: any locale carried.
            let any: Vec<String> = crate::api_rules::MOBILE_LOCALES
                .iter()
                .map(|locale| (*locale).to_owned())
                .collect();
            let locales = if locales.is_empty() { &any } else { locales };
            Some(alternatives(
                locales.iter().filter_map(|l| mobile_pattern(l)).collect(),
            ))
        }
        Rule::PostalCode(countries) => Some(alternatives(
            countries.iter().filter_map(|c| postal_pattern(c)).collect(),
        )),
        _ => None,
    }
}

/// `{column}_{rule}`, checking one rule.
fn push_check_function(output: &mut String, column: &Column, rule: &Rule, needs: &mut Needs) {
    output.push('\n');
    push_doc(
        output,
        0,
        &[&format!(
            "The `{}` rule of the {} column.",
            rule.display(),
            doc_name(&column.sql_name)
        )],
    );
    push_signature(
        output,
        0,
        &format!("fn {}", function_name(column, rule)),
        &["value: &str".to_owned()],
        "Result<(), validator::ValidationError>",
    );
    if let Some(pattern) = pattern(rule) {
        needs.insert(Helper::MatchesRegex);
        output.push_str(&format!(
            "    const PATTERN: &str = {};\n",
            str_literal(&pattern)
        ));
        output.push_str(
            "    static REGEX: std::sync::LazyLock<Option<regex::Regex>> =\n        std::sync::LazyLock::new(|| regex::Regex::new(PATTERN).ok());\n",
        );
    }
    let flag = |on: bool| if on { "true" } else { "false" };
    let valid = match rule {
        Rule::Uuid(versions) => {
            needs.insert(Helper::Uuid);
            let versions: Vec<String> = versions.iter().map(u8::to_string).collect();
            format!("is_uuid(value, &[{}])", versions.join(", "))
        }
        Rule::Url => {
            needs.insert(Helper::Url);
            "is_url(value)".to_owned()
        }
        Rule::Ulid => {
            needs.insert(Helper::Ulid);
            "is_ulid(value)".to_owned()
        }
        Rule::Mobile { strict: true, .. } => {
            output.push_str("    let strict = value.starts_with('+');\n");
            "strict && matches_regex(value, REGEX.as_ref())".to_owned()
        }
        Rule::Mobile { .. } | Rule::PostalCode(_) | Rule::Regex(_) => {
            "matches_regex(value, REGEX.as_ref())".to_owned()
        }
        Rule::Alpha(extras) | Rule::AlphaNumeric(extras) => {
            needs.insert(Helper::Letters);
            format!(
                "letters(value, {}, {}, {}, {})",
                flag(matches!(rule, Rule::AlphaNumeric(_))),
                flag(extras.spaces),
                flag(extras.dashes),
                flag(extras.underscores)
            )
        }
        Rule::Ascii => "!value.is_empty() && value.is_ascii()".to_owned(),
        Rule::In(values) | Rule::NotIn(values) => {
            push_str_array(output, 4, "VALUES", values);
            let not = if matches!(rule, Rule::NotIn(_)) {
                "!"
            } else {
                ""
            };
            format!("{not}VALUES.contains(&value)")
        }
        Rule::StartsWith(text) => {
            output.push_str(&format!(
                "    const PREFIX: &str = {};\n",
                str_literal(text)
            ));
            "value.starts_with(PREFIX)".to_owned()
        }
        Rule::EndsWith(text) => {
            output.push_str(&format!(
                "    const SUFFIX: &str = {};\n",
                str_literal(text)
            ));
            "value.ends_with(SUFFIX)".to_owned()
        }
        Rule::MinLength(count) => format!("value.chars().count() >= {count}"),
        Rule::MaxLength(count) => format!("value.chars().count() <= {count}"),
        Rule::FixedLength(count) => format!("value.chars().count() == {count}"),
        Rule::HexCode => {
            needs.insert(Helper::HexCode);
            "is_hex_code(value)".to_owned()
        }
        Rule::Iban => {
            needs.insert(Helper::Iban);
            "is_iban(value)".to_owned()
        }
        Rule::Jwt => {
            needs.insert(Helper::Jwt);
            "is_jwt(value)".to_owned()
        }
        Rule::Coordinates => {
            needs.insert(Helper::Coordinates);
            "is_coordinates(value)".to_owned()
        }
        Rule::CreditCard(providers) => {
            needs.insert(Helper::CreditCard);
            if providers.is_empty() {
                "is_credit_card(value, &[])".to_owned()
            } else {
                push_str_array(output, 4, "PROVIDERS", providers);
                "is_credit_card(value, PROVIDERS)".to_owned()
            }
        }
        _ => "true".to_owned(),
    };
    let args = [valid, format!("\"{}\"", rule.name())];
    push_call(output, 4, "", "check", &args, "");
    output.push_str("}\n");
}

/// validator.js's `isMobilePhone` pattern of `locale`, `\d` written
/// `[0-9]` (JavaScript's `\d` is ASCII).
fn mobile_pattern(locale: &str) -> Option<&'static str> {
    crate::api_rules::MOBILE_PATTERNS
        .iter()
        .find(|(key, _)| *key == locale)
        .map(|(_, pattern)| *pattern)
}

/// validator.js's `isPostalCode` pattern of `country`, `\d` written
/// `[0-9]`, and `/i` as `(?i)`.
fn postal_pattern(country: &str) -> Option<&'static str> {
    crate::api_rules::POSTAL_PATTERNS
        .iter()
        .find(|(key, _)| *key == country)
        .map(|(_, pattern)| *pattern)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_rules::{MOBILE_LOCALES, POSTAL_COUNTRIES};

    #[test]
    fn every_locale_and_country_has_a_pattern_that_parses() {
        for locale in MOBILE_LOCALES {
            let pattern = mobile_pattern(locale).expect(locale);
            regex_syntax::Parser::new().parse(pattern).expect(locale);
        }
        for country in POSTAL_COUNTRIES {
            let pattern = postal_pattern(country).expect(country);
            regex_syntax::Parser::new().parse(pattern).expect(country);
        }
    }

    /// `mobile` without a locale matches a number of any locale carried.
    #[test]
    fn mobile_without_a_locale_takes_every_locale() {
        let any = Rule::Mobile {
            locales: Vec::new(),
            strict: false,
        };
        let pattern = pattern(&any).expect("a pattern");
        regex_syntax::Parser::new()
            .parse(&pattern)
            .expect("any locale");
        for locale in MOBILE_LOCALES {
            let one = mobile_pattern(locale).expect(locale);
            assert!(pattern.contains(&format!("(?:{one})")), "{locale}");
        }
    }
}
