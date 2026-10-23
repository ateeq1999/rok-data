//! The `[generate.scopes]` configuration: the named scopes the generated
//! models declare (spec D7), checked and applied.
//!
//! A scope's free function is the app's code — the engine wires the
//! attribute onto the generated model (`#[rok(scopes(famous =
//! crate::scopes::users::famous))]`), and the derive's signature check
//! holds the function's shape `fn(&Select<M>) -> Select<M>` when the
//! consuming crate compiles. The engine refuses what it can catch at
//! generation time: an entry that names no table, a scope name that is not
//! a snake_case Rust identifier, or a path that cannot be one.

use std::collections::BTreeMap;

use crate::parser::naming::rust_identifier;
use crate::schema::{ScopeDeclaration, Table};

/// The configuration's scopes: by table (its SQL name, schema-qualified
/// name, or module name), then by scope name, with each scope's function
/// path.
pub type Scopes = BTreeMap<String, BTreeMap<String, String>>;

/// Checks `scopes` against the tables of the whole schema — the run's and
/// the kept ones included, so a run limited to some tables (`--resource`)
/// accepts the same configuration: an entry naming no table, a scope name
/// that is not a snake_case Rust identifier, or a function path that
/// cannot be one fails, naming the configuration entry.
pub fn check(tables: &[Table], scopes: &Scopes) -> Result<(), String> {
    if scopes.is_empty() {
        return Ok(());
    }
    for (key, scopes) in scopes {
        let at = location(key, None);
        if !tables.iter().any(|table| {
            table.display_name() == *key || table.module_name == *key || table.sql_name == *key
        }) {
            return Err(format!(
                "{at} in the configuration names no table of the schema"
            ));
        }
        for (name, path) in scopes {
            let at = location(key, Some(name));
            if rust_identifier(name).as_deref() != Ok(name.as_str()) {
                return Err(format!(
                    "{at} in the configuration names the scope `{name}`, which is not a snake_case Rust identifier"
                ));
            }
            if path.trim() != path || path.is_empty() || path.contains(char::is_whitespace) {
                return Err(format!(
                    "{at} in the configuration names the scope function `{path}`, which is not a Rust path"
                ));
            }
        }
    }
    Ok(())
}

/// Applies `scopes` to the tables of the run (`Table::scopes`): every scope
/// the configuration declares for a table, sorted by name. Entries for
/// tables outside the run are skipped — [`check`] has already checked them
/// against the whole schema.
pub fn apply(tables: &mut [Table], scopes: &Scopes) {
    for (key, scopes) in scopes {
        let Some(table) = tables.iter_mut().find(|table| {
            table.display_name() == *key || table.module_name == *key || table.sql_name == *key
        }) else {
            continue;
        };
        table.scopes = scopes
            .iter()
            .map(|(name, path)| ScopeDeclaration {
                name: name.clone(),
                path: path.clone(),
            })
            .collect();
    }
}

/// Where a `[generate.scopes]` entry lives, as a dotted key path:
/// `scopes.users.famous`.
fn location(table: &str, name: Option<&str>) -> String {
    let quote = |key: &str| {
        if key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            key.to_owned()
        } else {
            format!("\"{key}\"")
        }
    };
    match name {
        Some(name) => format!("`scopes.{}.{}`", quote(table), quote(name)),
        None => format!("`scopes.{}`", quote(table)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::test_support::{column, table};

    /// A table of one column, under both names the configuration may name
    /// it by.
    fn users() -> Table {
        table(vec![column("id", "i64")], &["id"])
    }

    #[test]
    fn empty_scopes_declare_nothing() {
        let mut tables = vec![users()];
        apply(&mut tables, &BTreeMap::new());
        assert!(tables[0].scopes.is_empty());
    }

    #[test]
    fn the_entries_name_the_scope_and_its_function() {
        let mut scopes: Scopes = BTreeMap::new();
        let mut named = BTreeMap::new();
        named.insert(
            "famous".to_owned(),
            "crate::scopes::users::famous".to_owned(),
        );
        scopes.insert("users".to_owned(), named);

        let mut tables = vec![users()];
        apply(&mut tables, &scopes);
        assert_eq!(
            tables[0].scopes,
            vec![ScopeDeclaration {
                name: "famous".to_owned(),
                path: "crate::scopes::users::famous".to_owned(),
            }]
        );
    }

    #[test]
    fn an_entry_names_a_table_by_its_module_name_too() {
        let mut scopes: Scopes = BTreeMap::new();
        let mut named = BTreeMap::new();
        named.insert("famous".to_owned(), "x::y::famous".to_owned());
        scopes.insert("users".to_owned(), named);

        let mut tables = vec![users()];
        apply(&mut tables, &scopes);
        assert_eq!(tables[0].scopes.len(), 1);
    }

    #[test]
    fn an_entry_for_a_table_outside_the_run_is_skipped() {
        let mut scopes: Scopes = BTreeMap::new();
        let mut named = BTreeMap::new();
        named.insert("famous".to_owned(), "x::y::famous".to_owned());
        scopes.insert("invoices".to_owned(), named);

        let mut tables = vec![users()];
        apply(&mut tables, &scopes);
        assert!(tables[0].scopes.is_empty());
    }

    #[test]
    fn a_scope_name_that_is_not_a_snake_case_identifier_fails() {
        let mut scopes: Scopes = BTreeMap::new();
        let mut named = BTreeMap::new();
        named.insert("Famous".to_owned(), "x::y::famous".to_owned());
        scopes.insert("users".to_owned(), named);

        let error = check(&[users()], &scopes).expect_err("the bad name fails");
        assert!(error.contains("scopes.users.Famous"), "{error}");
        assert!(
            error.contains("not a snake_case Rust identifier"),
            "{error}"
        );
    }

    #[test]
    fn an_unknown_table_fails_nameing_the_config_entry() {
        let mut scopes: Scopes = BTreeMap::new();
        let mut named = BTreeMap::new();
        named.insert("famous".to_owned(), "x::y::famous".to_owned());
        scopes.insert("invoices".to_owned(), named);

        let error = check(&[users()], &scopes).expect_err("the unknown table fails");
        assert!(error.contains("`scopes.invoices`"), "{error}");
        assert!(error.contains("names no table of the schema"), "{error}");
    }

    #[test]
    fn an_empty_path_fails() {
        let mut scopes: Scopes = BTreeMap::new();
        let mut named = BTreeMap::new();
        named.insert("famous".to_owned(), String::new());
        scopes.insert("users".to_owned(), named);

        let error = check(&[users()], &scopes).expect_err("the empty path fails");
        assert!(error.contains("`scopes.users.famous`"), "{error}");
        assert!(error.contains("not a Rust path"), "{error}");
    }
}
