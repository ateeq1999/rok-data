//! String rules (`[api.rules]`) on the create and update bodies: a field's
//! doc lines, the OpenAPI keywords utoipa can carry, and its
//! `#[rok(transform(…), validate(…))]` for `rok_data::Input`: the
//! runtime's catalogue rules, each answering its VineJS code.

use super::layout::push_field_attr;
use crate::api_rules::{Extras, Rule};
use crate::generator::rok_attr::Item;
use crate::generator::sql::str_literal;
use crate::generator::validate::field_rule;
use crate::generator::{MAX_WIDTH, push_doc};
use crate::schema::Column;

/// The doc lines a field's rules add: its transforms and its rules, after a
/// blank line. The OpenAPI document shows them as the field's description.
pub fn push_doc_lines(output: &mut String, rules: &[Rule]) {
    if rules.is_empty() {
        return;
    }
    push_doc(output, 4, &[""]);
    let names = |transforms: bool| -> Vec<String> {
        rules
            .iter()
            .filter(|rule| rule.is_transform() == transforms)
            .map(|rule| format!("`{}`", rule.display()))
            .collect()
    };
    for (label, names) in [("Transforms:", names(true)), ("Rules:", names(false))] {
        if names.is_empty() {
            continue;
        }
        // One line when it fits, otherwise wrapped at the maximum width.
        let mut line = label.to_owned();
        for (index, name) in names.iter().enumerate() {
            let last = index + 1 == names.len();
            let word = if last {
                format!("{name}.")
            } else {
                format!("{name},")
            };
            if 8 + line.len() + 1 + word.len() > MAX_WIDTH {
                push_doc(output, 4, &[&line]);
                line = word;
            } else {
                line.push(' ');
                line.push_str(&word);
            }
        }
        push_doc(output, 4, &[&line]);
    }
}

/// `#[schema(...)]` with the OpenAPI keywords utoipa can carry: a format
/// (`email`, `ipv4`, `ipv6`, `uuid`; not `uri` for `url`, which
/// takes a value without a scheme), a `pattern`, and lengths.
/// The first of each keyword wins.
pub fn push_schema(output: &mut String, openapi: bool, rules: &[Rule]) {
    if !openapi {
        return;
    }
    let mut format: Option<&str> = None;
    let mut pattern: Option<String> = None;
    let (mut min, mut max): (Option<u32>, Option<u32>) = (None, None);
    for rule in rules {
        match rule {
            Rule::Email => {
                format.get_or_insert("email");
            }
            Rule::IpAddress(Some(4)) => {
                format.get_or_insert("ipv4");
            }
            Rule::IpAddress(Some(_)) => {
                format.get_or_insert("ipv6");
            }
            Rule::Uuid(_) => {
                format.get_or_insert("uuid");
            }
            Rule::Regex(regex) => {
                pattern.get_or_insert_with(|| regex.clone());
            }
            Rule::MinLength(count) => {
                min.get_or_insert(*count);
            }
            Rule::MaxLength(count) => {
                max.get_or_insert(*count);
            }
            Rule::FixedLength(count) => {
                min.get_or_insert(*count);
                max.get_or_insert(*count);
            }
            _ => {}
        }
    }
    let mut args = Vec::new();
    if let Some(format) = format {
        args.push(format!("format = \"{format}\""));
    }
    if let Some(pattern) = pattern {
        args.push(format!("pattern = {}", str_literal(&pattern)));
    }
    if let Some(min) = min {
        args.push(format!("min_length = {min}"));
    }
    if let Some(max) = max {
        args.push(format!("max_length = {max}"));
    }
    if !args.is_empty() {
        push_field_attr(output, "schema", &args);
    }
}

/// The field's `#[rok(...)]` items: its transforms, then its rules — the
/// column's own `length` or `range` rule first, then each string rule as
/// the runtime's catalogue names it, answering its VineJS code.
pub fn rok_items(column: &Column, rules: &[Rule]) -> Vec<Item> {
    let transforms: Vec<Item> = rules
        .iter()
        .filter_map(|rule| match rule {
            Rule::Trim => Some("trim"),
            Rule::ToLowerCase => Some("lowercase"),
            Rule::ToUpperCase => Some("uppercase"),
            Rule::NormalizeEmail => Some("normalize_email"),
            _ => None,
        })
        .map(Item::word)
        .collect();
    let checks: Vec<Item> = field_rule(column)
        .into_iter()
        .chain(
            rules
                .iter()
                .filter(|rule| !rule.is_transform())
                .map(|rule| check(&column.sql_name, rule)),
        )
        .collect();
    let mut items = Vec::new();
    if !transforms.is_empty() {
        items.push(Item::list("transform", transforms));
    }
    if !checks.is_empty() {
        items.push(Item::list("validate", checks));
    }
    items
}

/// `rule` as a runtime rule. Every one but `email` answers the message the
/// API documents; `email` names it, since the runtime's own doesn't name
/// the field.
fn check(name: &str, rule: &Rule) -> Item {
    let quoted = |values: &[String]| -> String {
        let values: Vec<String> = values.iter().map(|value| str_literal(value)).collect();
        format!("[{}]", values.join(", "))
    };
    let flags = |extras: &Extras| -> Vec<Item> {
        [
            (extras.spaces, "spaces"),
            (extras.dashes, "dashes"),
            (extras.underscores, "underscores"),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, flag)| Item::word(flag))
        .collect()
    };
    let bare_or = |name: &str, items: Vec<Item>| {
        if items.is_empty() {
            Item::word(name)
        } else {
            Item::list(name, items)
        }
    };
    match rule {
        Rule::Email => Item::list(
            "email",
            vec![Item::word(format!(
                "message = {}",
                str_literal(&format!("{name} must be an email address"))
            ))],
        ),
        Rule::Url => Item::word("url"),
        Rule::Uuid(versions) if versions.is_empty() => Item::word("uuid"),
        Rule::Uuid(versions) => {
            let versions: Vec<String> = versions.iter().map(u8::to_string).collect();
            Item::list(
                "uuid",
                vec![Item::word(format!("versions = [{}]", versions.join(", ")))],
            )
        }
        Rule::Ulid => Item::word("ulid"),
        Rule::IpAddress(None) => Item::word("ip"),
        Rule::IpAddress(Some(version)) => {
            Item::list("ip", vec![Item::word(format!("version = {version}"))])
        }
        Rule::Mobile { locales, strict } => {
            let mut items = Vec::new();
            if !locales.is_empty() {
                items.push(Item::word(format!("locales = {}", quoted(locales))));
            }
            if *strict {
                items.push(Item::word("strict"));
            }
            bare_or("mobile", items)
        }
        Rule::Alpha(extras) => bare_or("alpha", flags(extras)),
        Rule::AlphaNumeric(extras) => bare_or("alpha_numeric", flags(extras)),
        Rule::Ascii => Item::word("ascii"),
        Rule::Regex(regex) => Item::word(format!("regex = {}", str_literal(regex))),
        Rule::In(values) => Item::list(
            "one_of",
            vec![Item::word(format!("values = {}", quoted(values)))],
        ),
        Rule::NotIn(values) => Item::list(
            "not_one_of",
            vec![Item::word(format!("values = {}", quoted(values)))],
        ),
        Rule::StartsWith(text) => Item::word(format!("starts_with = {}", str_literal(text))),
        Rule::EndsWith(text) => Item::word(format!("ends_with = {}", str_literal(text))),
        Rule::MinLength(count) => Item::word(format!("min_length = {count}")),
        Rule::MaxLength(count) => Item::word(format!("max_length = {count}")),
        Rule::FixedLength(count) => Item::word(format!("fixed_length = {count}")),
        Rule::HexCode => Item::word("hex_code"),
        Rule::Iban => Item::word("iban"),
        Rule::Jwt => Item::word("jwt"),
        Rule::PostalCode(countries) => Item::list(
            "postal_code",
            vec![Item::word(format!("countries = {}", quoted(countries)))],
        ),
        Rule::CreditCard(providers) if providers.is_empty() => Item::word("credit_card"),
        Rule::CreditCard(providers) => Item::list(
            "credit_card",
            vec![Item::word(format!("providers = {}", quoted(providers)))],
        ),
        Rule::Coordinates => Item::word("coordinates"),
        Rule::Trim | Rule::ToLowerCase | Rule::ToUpperCase | Rule::NormalizeEmail => {
            unreachable!("a transform is not a check")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_read_as_the_runtime_catalogue() {
        let item = |rule: Rule| check("handle", &rule).one_line();
        assert_eq!(
            item(Rule::AlphaNumeric(Extras {
                spaces: false,
                dashes: true,
                underscores: true
            })),
            "alpha_numeric(dashes, underscores)"
        );
        assert_eq!(item(Rule::FixedLength(1)), "fixed_length = 1");
        assert_eq!(
            item(Rule::In(vec!["SA".into(), "AE".into()])),
            "one_of(values = [\"SA\", \"AE\"])"
        );
        assert_eq!(
            item(Rule::Email),
            "email(message = \"handle must be an email address\")"
        );
    }
}
