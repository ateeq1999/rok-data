//! What `#[derive(Model)]` infers on its own (workspace ADR 0015), so the
//! generator writes only what differs (0.3.0 spec D4): the table name from
//! the struct's name, and the primary key from a field named `id`.
//!
//! `snake_case` and `pluralize` mirror rok-data-macros' `naming` module
//! character for character — a proc-macro crate exports no functions, so
//! the two copies are pinned by the tests below and the derive's own.

use crate::schema::Table;

/// The table the derive infers for `table`'s row: the struct's name in
/// snake_case, pluralized. Matches when no `#[rok(table = …)]` is needed.
pub(super) fn infers_table(table: &Table) -> bool {
    table.schema.is_none() && pluralize(&snake_case(&table.struct_name)) == table.sql_name
}

/// Whether the derive infers `table`'s primary key: a single key column
/// whose field is named `id` (the derive picks `id` when no field is
/// marked `#[rok(primary_key)]`).
pub(super) fn infers_primary_key(table: &Table) -> bool {
    matches!(table.primary_key_columns().as_slice(), [only] if only.rust_name == "id")
}

fn snake_case(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let prev_lower =
                i > 0 && (chars[i - 1].is_lowercase() || chars[i - 1].is_ascii_digit());
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            let prev_upper = i > 0 && chars[i - 1].is_uppercase();
            if prev_lower || (prev_upper && next_lower) {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn pluralize(s: &str) -> String {
    let ends_with_consonant_y =
        s.ends_with('y') && !s.chars().rev().nth(1).is_some_and(|c| "aeiou".contains(c));
    if ends_with_consonant_y {
        format!("{}ies", &s[..s.len() - 1])
    } else if ["s", "x", "z", "ch", "sh"].iter().any(|e| s.ends_with(e)) {
        format!("{s}es")
    } else {
        format!("{s}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_derives_table_names() {
        for (model, table) in [
            ("User", "users"),
            ("Category", "categories"),
            ("Day", "days"),
            ("Box", "boxes"),
            ("Branch", "branches"),
            ("AuditLog", "audit_logs"),
            ("HTTPRequest", "http_requests"),
            ("Address", "addresses"),
        ] {
            assert_eq!(pluralize(&snake_case(model)), table, "{model}");
        }
    }
}
