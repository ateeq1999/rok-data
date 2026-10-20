//! The checks a parsed schema must pass.

use sqlparser::ast::{Expr, Ident, ObjectName};

use crate::{Error, naming};

use crate::parse::Parsed;

/// Cross-file checks: references and names.
pub(crate) fn check(parsed: &Parsed) -> Result<(), Error> {
    let schema = &parsed.schema;
    for module in &parsed.modules {
        if naming::RESERVED_MODULES.contains(&module.as_str()) {
            return Err(Error::new(format!(
                "`{module}.sql` would generate `{module}.rs`, which rok writes itself; rename the file"
            )));
        }
    }
    for table in &schema.tables {
        for fk in &table.foreign_keys {
            let Some(target) = schema.tables.iter().find(|t| t.name == fk.ref_table) else {
                return Err(Error::new(format!(
                    "`{}` references `{}`, which no .sql file declares",
                    table.name, fk.ref_table
                )));
            };
            if fk.ref_columns.is_empty() && target.primary_key.is_empty() {
                return Err(Error::new(format!(
                    "`{}` references `{}`, which has no primary key",
                    table.name, fk.ref_table
                )));
            }
        }
    }
    let mut structs: Vec<(String, &str)> = Vec::new();
    for table in &schema.tables {
        let name = naming::struct_name(&table.name);
        if let Some((_, other)) = structs.iter().find(|(n, _)| *n == name) {
            return Err(Error::new(format!(
                "tables `{other}` and `{}` both become `{name}`; rename one",
                table.name
            )));
        }
        structs.push((name, &table.name));
    }
    for e in &schema.enums {
        let name = naming::pascal(naming::unqualified(&e.name));
        if structs.iter().any(|(n, _)| *n == name) {
            return Err(Error::new(format!(
                "enum `{}` and a table both become `{name}`; rename one",
                e.name
            )));
        }
    }
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for q in &parsed.queries {
        if seen.contains(&(q.module.as_str(), q.name.as_str())) {
            return Err(Error::at(
                &q.location,
                format!("a query named `{}` already exists in this file", q.name),
            ));
        }
        seen.push((&q.module, &q.name));
    }
    Ok(())
}

pub(crate) fn constraint_name(name: Option<&Ident>, default: impl FnOnce() -> String) -> String {
    name.map_or_else(default, ident)
}

/// An identifier as PostgreSQL stores it: unquoted names fold to lower case.
pub(crate) fn ident(i: &Ident) -> String {
    if i.quote_style.is_some() {
        i.value.clone()
    } else {
        i.value.to_ascii_lowercase()
    }
}

pub(crate) fn expr_ident(expr: &Expr) -> String {
    match expr {
        Expr::Identifier(i) => ident(i),
        other => other.to_string(),
    }
}

/// `"app"."Users"` -> `app.Users`; unquoted parts fold to lower case.
pub(crate) fn object_name(name: &ObjectName) -> String {
    name.0
        .iter()
        .map(|part| part.as_ident().map_or_else(|| part.to_string(), ident))
        .collect::<Vec<_>>()
        .join(".")
}

/// Quote a possibly schema-qualified name for SQL.
pub(crate) fn quote(name: &str) -> String {
    name.split('.')
        .map(|part| format!("\"{}\"", part.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(".")
}
