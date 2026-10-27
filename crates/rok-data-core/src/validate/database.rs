//! Database rules: an input's `unique` and `exists` checks, run together
//! in one statement against the session `validate` takes (workspace
//! ADR 0021).

use std::fmt;

use sqlx::Row;

use crate::exec::Journey;
use crate::sql::Sql;
use crate::{Column, Expr, Model, Result, Select, Value};

use super::ValidationErrors;

/// One built-in database rule of an input field, made by
/// `#[derive(Input)]` from `unique(…)` / `exists(…)`: an `EXISTS` over the
/// target table, and whether finding a row is the failure (`unique`) or
/// not finding one is (`exists`).
pub struct DatabaseRule {
    pub(crate) field: &'static str,
    code: &'static str,
    message: String,
    fails_when_found: bool,
    body: Box<dyn Fn(&mut Sql) + Send + Sync>,
}

impl fmt::Debug for DatabaseRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sql = Sql::new();
        (self.body)(&mut sql);
        f.debug_struct("DatabaseRule")
            .field("field", &self.field)
            .field("code", &self.code)
            .field("exists", &sql.as_str())
            .finish()
    }
}

impl DatabaseRule {
    fn new<T: Model>(
        field: &'static str,
        code: &'static str,
        message: String,
        fails_when_found: bool,
        select: Select<T>,
    ) -> Self {
        Self {
            field,
            code,
            message,
            fails_when_found,
            body: Box::new(move |sql| select.write_exists_body(sql)),
        }
    }

    /// `unique(T::COLUMN, …)`: no row of `T` may hold `value` in `column`
    /// (ignoring case when `case_insensitive`), other than the row whose
    /// key is `except`, among the rows `scope` and `T`'s own scopes leave.
    /// `None` when the value is `NULL`: there is nothing to check.
    #[doc(hidden)]
    pub fn unique<T: Model>(
        field: &'static str,
        column: Column<T>,
        value: Value,
        case_insensitive: bool,
        except: Option<Value>,
        scope: Option<Expr<T>>,
    ) -> Option<Self> {
        if value.is_null() {
            return None;
        }
        let matching = if case_insensitive {
            column.eq_ignore_case(value)
        } else {
            column.eq(value)
        };
        let mut select = T::filter(matching);
        if let Some(except) = except.filter(|v| !v.is_null()) {
            select = select.filter(T::primary_key_column().ne(except));
        }
        if let Some(scope) = scope {
            select = select.filter(scope);
        }
        let message = format!("{field} is already taken");
        Some(Self::new(field, "unique", message, true, select))
    }

    /// `exists(T::COLUMN, …)`: some row of `T` must hold `value` in
    /// `column`, among the rows `scope` and `T`'s own scopes leave. `None`
    /// when the value is `NULL`.
    #[doc(hidden)]
    pub fn exists<T: Model>(
        field: &'static str,
        column: Column<T>,
        value: Value,
        scope: Option<Expr<T>>,
    ) -> Option<Self> {
        if value.is_null() {
            return None;
        }
        let mut select = T::filter(column.eq(value));
        if let Some(scope) = scope {
            select = select.filter(scope);
        }
        let message = format!(
            "{field} does not refer to an existing row of `{}`",
            T::TABLE
        );
        Some(Self::new(field, "exists", message, false, select))
    }
}

/// `SELECT EXISTS(…) AS "r0", EXISTS(…) AS "r1", …`: every rule in one
/// statement.
pub(crate) fn batch_sql(rules: &[DatabaseRule]) -> Sql {
    let mut sql = Sql::new();
    sql.push("SELECT ");
    sql.push_list(rules.iter().enumerate(), ", ", |sql, (i, rule)| {
        sql.push("EXISTS(");
        (rule.body)(sql);
        sql.push(") AS ").push_ident(&format!("r{i}"));
    });
    sql
}

/// Runs `rules` in one statement on `journey` and returns their failures.
pub(crate) async fn run(
    journey: &mut Journey<'_>,
    rules: Vec<DatabaseRule>,
) -> Result<ValidationErrors> {
    let mut errors = ValidationErrors::new();
    if rules.is_empty() {
        return Ok(errors);
    }
    let sql = batch_sql(&rules);
    let rows = crate::query::with::fetch_rows_on(journey, &sql).await?;
    let Some(row) = rows.first() else {
        return Ok(errors);
    };
    for (i, rule) in rules.into_iter().enumerate() {
        let found: bool = row.try_get(i)?;
        if found == rule.fails_when_found {
            errors.add(rule.field, rule.code, rule.message);
        }
    }
    Ok(errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Column;

    #[derive(Debug, Clone)]
    struct User {
        id: i64,
        email: String,
    }

    impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for User {
        fn from_row(_row: &'r sqlx::postgres::PgRow) -> sqlx::Result<Self> {
            unreachable!("the test model is never built from rows")
        }
    }

    impl crate::Hooks for User {}

    impl Model for User {
        const TABLE: &'static str = "users";
        const PRIMARY_KEY: &'static str = "id";
        const COLUMNS: &'static [&'static str] = &["id", "email"];
        fn primary_key(&self) -> Value {
            Value::from(self.id)
        }
        fn values(&self) -> Vec<(&'static str, Value)> {
            vec![
                ("id", Value::from(self.id)),
                ("email", Value::from(self.email.clone())),
            ]
        }
    }

    #[test]
    fn rules_batch_into_one_statement() {
        let rules: Vec<DatabaseRule> = [
            DatabaseRule::unique(
                "email",
                Column::<User>::new("email"),
                Value::from("a@b.io"),
                true,
                Some(Value::from(7_i64)),
                None,
            ),
            DatabaseRule::exists(
                "owner_id",
                Column::<User>::new("id"),
                Value::from(3_i64),
                None,
            ),
            DatabaseRule::exists(
                "nobody",
                Column::<User>::new("id"),
                Value::from(None::<i64>),
                None,
            ),
        ]
        .into_iter()
        .flatten()
        .collect();
        assert_eq!(rules.len(), 2, "a NULL value has nothing to check");
        assert_eq!(
            batch_sql(&rules).as_str(),
            concat!(
                r#"SELECT EXISTS(SELECT 1 FROM "users" WHERE (lower("users"."email") = lower($1)) AND "id" <> $2) AS "r0", "#,
                r#"EXISTS(SELECT 1 FROM "users" WHERE "id" = $3) AS "r1""#,
            )
        );
    }
}
