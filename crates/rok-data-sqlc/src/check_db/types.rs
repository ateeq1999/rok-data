//! Which PostgreSQL types a generated Rust type binds and decodes.

use std::collections::HashMap;

use crate::schema::{EnumType, Table, rust_type};

/// The types the schema defines: typed ids (by name, to the type they wrap)
/// and enums (by Rust path, to the database types they encode as).
pub(super) struct Known {
    typed_ids: HashMap<String, String>,
    enums: HashMap<String, Vec<String>>,
}

impl Known {
    pub(super) fn new(tables: &[Table], enums: &[EnumType]) -> Self {
        let typed_ids = tables
            .iter()
            .flat_map(|table| &table.columns)
            .filter_map(|column| {
                let id = column.typed_id.as_ref()?;
                Some((id.name.clone(), column.sqlx_type.clone()))
            })
            .collect();
        let enums = enums
            .iter()
            .map(|enum_type| {
                // A value-list column's enum encodes as text.
                let names = match &enum_type.column {
                    Some(_) => text(),
                    None => vec![enum_type.sql_name.to_ascii_uppercase()],
                };
                (enum_type.rust_path(), names)
            })
            .collect();
        Self { typed_ids, enums }
    }

    /// Whether `rust` binds or decodes as the database type `db_type`;
    /// `None` when the type is not one the check knows.
    pub(super) fn accepts(&self, rust: &str, db_type: &str) -> Option<bool> {
        let db_type = leaf(db_type).to_ascii_uppercase();
        let names = self.names(&base(rust))?;
        Some(names.contains(&db_type))
    }

    /// The database type names a base (unwrapped, unborrowed) type takes.
    fn names(&self, base: &str) -> Option<Vec<String>> {
        if base == rust_type::BYTES {
            return Some(vec!["BYTEA".to_owned()]);
        }
        if let Some(element) = generic_arg(base, "Vec") {
            let names = self.names(element)?;
            return Some(names.into_iter().map(|name| format!("{name}[]")).collect());
        }
        if let Some(names) = primitive(base) {
            return Some(names.iter().map(|name| (*name).to_owned()).collect());
        }
        if let Some(names) = self.enums.get(base) {
            return Some(names.clone());
        }
        let name = base.rsplit("::").next().unwrap_or(base);
        let inner = self.typed_ids.get(name)?;
        self.names(inner)
    }
}

/// Whether `rust` is an `Option`.
pub(super) fn is_option(rust: &str) -> bool {
    generic_arg(rust.trim(), "Option").is_some()
}

/// `rust` without a borrow or `Option`: `&str` is `String`, `&[T]` is
/// `Vec<T>`.
fn base(rust: &str) -> String {
    let rust = rust.trim();
    if let Some(inner) = generic_arg(rust, "Option") {
        return base(inner);
    }
    let rust = rust.trim_start_matches('&').trim();
    match rust {
        "str" => "String".to_owned(),
        _ => match rust.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            Some(element) => format!("Vec<{}>", base(element)),
            None => rust.to_owned(),
        },
    }
}

/// `T` of `Name<T>` (or of `std::…::Name<T>`).
fn generic_arg<'a>(rust: &'a str, name: &str) -> Option<&'a str> {
    let open = rust.find('<')?;
    let head = &rust[..open];
    (head == name || head.ends_with(&format!("::{name}")))
        .then(|| rust[open + 1..].strip_suffix('>'))
        .flatten()
        .map(str::trim)
}

/// A type name without its schema or quotes: `app.status` is `status`.
fn leaf(db_type: &str) -> &str {
    db_type
        .rsplit('.')
        .next()
        .unwrap_or(db_type)
        .trim_matches('"')
}

fn text() -> Vec<String> {
    ["TEXT", "VARCHAR", "BPCHAR", "CHAR", "NAME", "CITEXT"]
        .map(str::to_owned)
        .to_vec()
}

fn primitive(base: &str) -> Option<&'static [&'static str]> {
    let names: &[&str] = match base {
        "i16" => &["INT2"],
        "i32" => &["INT4"],
        "i64" => &["INT8"],
        "bool" => &["BOOL"],
        "f32" => &["FLOAT4"],
        "f64" => &["FLOAT8"],
        "String" => &["TEXT", "VARCHAR", "BPCHAR", "CHAR", "NAME", "CITEXT"],
        "uuid::Uuid" => &["UUID"],
        "chrono::NaiveDate" => &["DATE"],
        "chrono::NaiveTime" => &["TIME"],
        "chrono::NaiveDateTime" => &["TIMESTAMP"],
        "chrono::DateTime<chrono::Utc>" => &["TIMESTAMPTZ"],
        "serde_json::Value" => &["JSON", "JSONB"],
        "rust_decimal::Decimal" => &["NUMERIC"],
        rust_type::INTERVAL => &["INTERVAL"],
        rust_type::TIME_TZ => &["TIMETZ"],
        rust_type::MONEY => &["MONEY"],
        rust_type::IP_NETWORK => &["INET", "CIDR"],
        rust_type::MAC_ADDRESS => &["MACADDR"],
        "sqlx::postgres::types::PgRange<i32>" => &["INT4RANGE"],
        "sqlx::postgres::types::PgRange<i64>" => &["INT8RANGE"],
        "sqlx::postgres::types::PgRange<rust_decimal::Decimal>" => &["NUMRANGE"],
        "sqlx::postgres::types::PgRange<chrono::NaiveDate>" => &["DATERANGE"],
        "sqlx::postgres::types::PgRange<chrono::NaiveDateTime>" => &["TSRANGE"],
        "sqlx::postgres::types::PgRange<chrono::DateTime<chrono::Utc>>" => &["TSTZRANGE"],
        _ => return None,
    };
    Some(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> Known {
        Known {
            typed_ids: [("UserId".to_owned(), "i64".to_owned())].into(),
            enums: [("types::Role".to_owned(), vec!["USER_ROLE".to_owned()])].into(),
        }
    }

    #[test]
    fn borrows_options_and_slices_reduce_to_their_base() {
        let known = known();
        assert_eq!(known.accepts("&str", "TEXT"), Some(true));
        assert_eq!(known.accepts("Option<String>", "VARCHAR"), Some(true));
        assert_eq!(known.accepts("&[i64]", "INT8[]"), Some(true));
        assert_eq!(known.accepts("&[u8]", "BYTEA"), Some(true));
        assert_eq!(known.accepts("Vec<u8>", "BYTEA"), Some(true));
        assert_eq!(known.accepts("i32", "INT8"), Some(false));
    }

    #[test]
    fn typed_ids_and_enums_resolve_through_the_schema() {
        let known = known();
        assert_eq!(known.accepts("UserId", "INT8"), Some(true));
        assert_eq!(
            known.accepts("crate::db::users::UserId", "INT4"),
            Some(false)
        );
        assert_eq!(known.accepts("types::Role", "user_role"), Some(true));
        assert_eq!(known.accepts("types::Role", "public.user_role"), Some(true));
        assert_eq!(known.accepts("Option<types::Role>", "TEXT"), Some(false));
        assert_eq!(known.accepts("my::Thing", "TEXT"), None);
    }

    #[test]
    fn option_is_seen_through_paths() {
        assert!(is_option("Option<i64>"));
        assert!(is_option("std::option::Option<i64>"));
        assert!(!is_option("i64"));
    }
}
