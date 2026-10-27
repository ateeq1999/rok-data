//! The constraint backstop: a unique or foreign-key violation on a model
//! write reads as the field error a rule would have reported (workspace
//! ADR 0021). The table's constraint is the guarantee; this keeps a race
//! the rule lost from surfacing as a bare database error.

use sqlx::postgres::PgDatabaseError;

use crate::validate::ValidationErrors;
use crate::{Error, Model};

/// `error`, or — when it is a unique (`23505`) or foreign-key (`23503`)
/// violation on columns of `M` — an [`Error::Validation`] naming them, code
/// `unique` / `exists`. A violation PostgreSQL doesn't name the columns
/// of, or on columns `M` has no field for, stays a database error.
pub(crate) fn as_field_error<M: Model>(error: Error) -> Error {
    let Error::Database(sqlx::Error::Database(db)) = &error else {
        return error;
    };
    let unique = db.is_unique_violation();
    if !unique && !db.is_foreign_key_violation() {
        return error;
    }
    let Some(detail) = db
        .try_downcast_ref::<PgDatabaseError>()
        .and_then(PgDatabaseError::detail)
    else {
        return error;
    };
    let Some(columns) = key_columns(detail) else {
        return error;
    };
    if columns.is_empty() || !columns.iter().all(|c| M::COLUMNS.contains(&c.as_str())) {
        return error;
    }
    let mut errors = ValidationErrors::new();
    let Some((last, init)) = columns.split_last() else {
        return error;
    };
    if unique {
        let message = if init.is_empty() {
            format!("{last} is already taken")
        } else {
            format!("{} and {last} are already taken together", init.join(", "))
        };
        errors.add(last.clone(), "unique", message);
    } else {
        let table = referenced_table(detail).unwrap_or("the referenced table");
        for column in &columns {
            errors.add(
                column.clone(),
                "exists",
                format!("{column} does not refer to an existing row of `{table}`"),
            );
        }
    }
    Error::Validation(errors)
}

/// The columns of `Key (a, b)=(…) …`, PostgreSQL's violation detail.
fn key_columns(detail: &str) -> Option<Vec<String>> {
    let rest = detail.strip_prefix("Key (")?;
    let (columns, _) = rest.split_once(")=(")?;
    Some(
        columns
            .split(", ")
            .map(|c| c.trim_matches('"').to_owned())
            .collect(),
    )
}

/// The table of `… is not present in table "orgs".`
fn referenced_table(detail: &str) -> Option<&str> {
    let (_, rest) = detail.split_once("in table \"")?;
    rest.split_once('"').map(|(table, _)| table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_detail_names_the_key_columns_and_table() {
        assert_eq!(
            key_columns("Key (email)=(a@b.c) already exists."),
            Some(vec!["email".to_owned()])
        );
        assert_eq!(
            key_columns("Key (project_id, title)=(1, x) already exists."),
            Some(vec!["project_id".to_owned(), "title".to_owned()])
        );
        assert_eq!(key_columns("something else"), None);
        assert_eq!(
            referenced_table(r#"Key (org_id)=(5) is not present in table "orgs"."#),
            Some("orgs")
        );
    }
}
