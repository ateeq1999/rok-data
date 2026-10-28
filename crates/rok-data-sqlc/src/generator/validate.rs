//! Validation rules (engine ADR 0015, ADR 0025): the `#[rok(validate(...))]`
//! rules `derive = ["validate"]` puts on create input and update patch
//! fields, which derive `rok_data::Input`, derived from each column's
//! declared length and range checks.

use crate::schema::{BaseType, Column, ValueRange};

use super::rok_attr::{Item, push_rok};
use super::sql::str_literal;

/// Appends `rok_data::validate::HasLength` for the typed id `name` over
/// text, measuring the text inside, so a `length` rule applies through the
/// id.
pub fn push_measured_id(output: &mut String, name: &str) {
    output.push_str(&format!(
        "impl rok_data::validate::HasLength for {name} {{\n    fn length(&self) -> Option<usize> {{\n        rok_data::validate::HasLength::length(&self.0)\n    }}\n}}\n\n"
    ));
}

/// Appends `rok_data::validate::AsNumber` for the typed id `name` over a
/// number, comparing the number inside, so a `range` rule applies through
/// the id.
pub fn push_ranged_id(output: &mut String, name: &str) {
    output.push_str(&format!(
        "impl rok_data::validate::AsNumber for {name} {{\n    fn as_number(&self) -> Option<f64> {{\n        rok_data::validate::AsNumber::as_number(&self.0)\n    }}\n}}\n\n"
    ));
}

/// Appends the column's validation rule, if it has one, as a field
/// attribute. Patch fields (`Option<T>`, `Option<Option<T>>`) are checked
/// only when a value is present.
pub fn push_field_rule(output: &mut String, column: &Column) {
    if let Some(rule) = field_rule(column) {
        push_rok(output, vec![Item::list("validate", vec![rule])]);
    }
}

/// The column's `length` or `range` rule, if it has one.
pub fn field_rule(column: &Column) -> Option<Item> {
    length_rule(column).or_else(|| range_rule(column))
}

/// The `range` rule of a numeric column, from its range checks; a typed id
/// over a number takes it through `AsNumber` (see [`push_ranged_id`]).
fn range_rule(column: &Column) -> Option<Item> {
    let range = column.value_range.as_ref()?;
    let base = column.base_type();
    if !matches!(
        base,
        BaseType::Integer | BaseType::Float | BaseType::Decimal
    ) {
        return None;
    }
    let mut args = Vec::new();
    for (name, bound) in [("min", &range.min), ("max", &range.max)] {
        if let Some(bound) = bound {
            let exclusive = if bound.exclusive { "exclusive_" } else { "" };
            args.push(Item::word(format!("{exclusive}{name} = {}", bound.value)));
        }
    }
    let message = format!("{} must be {}", column.sql_name, range_phrase(range));
    args.push(Item::word(format!("message = {}", str_literal(&message))));
    Some(Item::list("range", args))
}

/// `"between 1 and 5"`, `"at least 0"`, `"greater than 0 and at most 1"`.
fn range_phrase(range: &ValueRange) -> String {
    let lower = range.min.as_ref().map(|bound| {
        let word = if bound.exclusive {
            "greater than"
        } else {
            "at least"
        };
        format!("{word} {}", bound.value)
    });
    let upper = range.max.as_ref().map(|bound| {
        let word = if bound.exclusive {
            "less than"
        } else {
            "at most"
        };
        format!("{word} {}", bound.value)
    });
    match (&range.min, &range.max) {
        (Some(min), Some(max)) if !min.exclusive && !max.exclusive => {
            format!("between {} and {}", min.value, max.value)
        }
        _ => [lower, upper]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" and "),
    }
}

/// The `length` rule of a text column: its declared length and length
/// checks, the tightest bounds when both apply. Lengths count characters,
/// as PostgreSQL counts them.
fn length_rule(column: &Column) -> Option<Item> {
    if column.base_type() != BaseType::Text {
        return None;
    }
    let checked = column.length_range;
    let min = checked.and_then(|range| range.min);
    let max = [column.declared_length, checked.and_then(|range| range.max)]
        .into_iter()
        .flatten()
        .min();
    let name = &column.sql_name;
    let (mut args, message) = match (min, max) {
        (None, None) => return None,
        (Some(min), Some(max)) if min == max => (
            vec![format!("min = {min}"), format!("max = {max}")],
            format!("{name} must be exactly {}", characters(min)),
        ),
        (Some(min), Some(max)) => (
            vec![format!("min = {min}"), format!("max = {max}")],
            format!("{name} must be between {min} and {}", characters(max)),
        ),
        (Some(min), None) => (
            vec![format!("min = {min}")],
            format!("{name} must be at least {}", characters(min)),
        ),
        (None, Some(max)) => (
            vec![format!("max = {max}")],
            format!("{name} must be at most {}", characters(max)),
        ),
    };
    args.push(format!("message = {}", str_literal(&message)));
    Some(Item::list(
        "length",
        args.into_iter().map(Item::word).collect(),
    ))
}

/// `"1 character"`, `"80 characters"`.
fn characters(count: u32) -> String {
    if count == 1 {
        "1 character".to_owned()
    } else {
        format!("{count} characters")
    }
}
