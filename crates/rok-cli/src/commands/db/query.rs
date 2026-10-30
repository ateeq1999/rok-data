//! `rok db query <SQL>`: every statement in one transaction that commits
//! only when all of them succeed (ADR 0028).

use std::process::ExitCode;

use rok_data::TryStreamExt;
use rok_data::sqlx::postgres::{PgConnection, PgRow};
use rok_data::sqlx::{
    AssertSqlSafe, Column, Connection as _, Either, Executor, Row, SqlSafeStr, TypeInfo,
};
use serde_json::{Value, json};

use super::split;
use crate::context;
use crate::output::json_line;

/// What one statement gave back.
enum Outcome {
    /// A statement with a result shape (`SELECT`, `… RETURNING`, `SHOW`):
    /// its columns and every row, each value as PostgreSQL's text for it.
    Rows {
        columns: Vec<(String, String)>,
        rows: Vec<Vec<Option<String>>>,
    },
    /// A statement without one: how many rows it touched.
    Affected(u64),
}

/// `rok db query`: split `sql` into statements, run them in one
/// transaction, commit when every one succeeds and roll back on the first
/// failure.
pub(crate) async fn query(
    sql: &str,
    database_url: Option<&str>,
    server: Option<&str>,
    json: bool,
) -> Result<ExitCode, String> {
    let statements = split::statements(sql);
    if statements.is_empty() {
        return Err("no SQL statement to run".to_owned());
    }
    if let Some((index, keyword)) = statements
        .iter()
        .enumerate()
        .find_map(|(index, sql)| transaction_control(sql).map(|keyword| (index, keyword)))
    {
        return Err(format!(
            "statement {n} is a `{keyword}`: `rok db query` runs every statement \
             in one transaction of its own; leave out the transaction control",
            n = index + 1
        ));
    }

    let connection = context::Connection::resolve_target(database_url, server)?;
    let mut conn = connection.connect().await?;
    let mut tx = conn
        .begin()
        .await
        .map_err(|e| format!("can't begin a transaction: {e}"))?;

    let mut outcomes = Vec::with_capacity(statements.len());
    for (index, statement) in statements.iter().enumerate() {
        match run_statement(&mut tx, statement).await {
            Ok(outcome) => outcomes.push(outcome),
            Err(error) => {
                // The original error is the one to report; dropping an
                // un-rolled-back transaction also ends it on the server.
                let rolled_back = tx.rollback().await.is_ok();
                let message = error_message(&error);
                if json {
                    json_line(json!({
                        "statements": statements.len(),
                        "committed": false,
                        "error": { "statement": index + 1, "message": message },
                    }));
                }
                let ending = if rolled_back {
                    "Transaction rolled back."
                } else {
                    "Transaction not committed (the connection was closed)."
                };
                return Err(format!(
                    "statement {n} failed: {message}\n{ending}",
                    n = index + 1
                ));
            }
        }
    }
    tx.commit()
        .await
        .map_err(|e| format!("can't commit the transaction: {}", error_message(&e)))?;

    if json {
        json_line(json!({
            "statements": statements.len(),
            "results": statements.iter().zip(&outcomes).map(|(sql, outcome)| outcome_json(sql, outcome)).collect::<Vec<_>>(),
            "committed": true,
        }));
    } else {
        for outcome in &outcomes {
            if let Outcome::Rows { columns, rows } = outcome {
                print_table(columns, rows);
            }
        }
        let count = statements.len();
        let noun = if count == 1 {
            "statement"
        } else {
            "statements"
        };
        println!("OK: {count} {noun} executed, transaction committed.");
    }
    Ok(ExitCode::SUCCESS)
}

/// Runs one statement inside the transaction. Its result shape comes from
/// describing it first, so a `SELECT` with no rows still has its columns;
/// it then runs over the simple protocol, which returns every value as
/// text.
async fn run_statement(
    conn: &mut PgConnection,
    statement: &str,
) -> Result<Outcome, rok_data::sqlx::Error> {
    let described = (&mut *conn)
        .describe(AssertSqlSafe(statement.to_owned()).into_sql_str())
        .await?;
    let columns: Vec<(String, String)> = described
        .columns()
        .iter()
        .map(|column| {
            (
                column.name().to_owned(),
                column.type_info().name().to_owned(),
            )
        })
        .collect();

    let mut rows = Vec::new();
    let mut affected = 0;
    let mut results =
        rok_data::sqlx::raw_sql(AssertSqlSafe(statement.to_owned())).fetch_many(&mut *conn);
    while let Some(step) = results.try_next().await? {
        match step {
            Either::Left(done) => affected += done.rows_affected(),
            Either::Right(row) => rows.push(text_values(&row)),
        }
    }
    Ok(if columns.is_empty() {
        Outcome::Affected(affected)
    } else {
        Outcome::Rows { columns, rows }
    })
}

/// A row's values as text; `None` for SQL `NULL`. The simple protocol sends
/// every value in its text form, which decodes as a string whatever the
/// column's type.
fn text_values(row: &PgRow) -> Vec<Option<String>> {
    (0..row.len())
        .map(|index| {
            row.try_get_unchecked::<Option<String>, _>(index)
                .ok()
                .flatten()
        })
        .collect()
}

/// PostgreSQL's own message when the database refused the statement;
/// sqlx's description of the failure otherwise.
fn error_message(error: &rok_data::sqlx::Error) -> String {
    match error.as_database_error() {
        Some(database) => database.message().to_owned(),
        None => error.to_string(),
    }
}

/// The transaction-control keyword `sql` starts with, if it would end or
/// nest the transaction `rok db query` runs it in. `ROLLBACK TO
/// SAVEPOINT` stays inside it, so it is allowed.
fn transaction_control(sql: &str) -> Option<String> {
    let mut words = code_words(sql);
    let first = words.next()?.to_ascii_uppercase();
    let second = words.next().map(|w| w.to_ascii_uppercase());
    let controls = match first.as_str() {
        "BEGIN" | "START" | "COMMIT" | "END" | "ABORT" => true,
        "ROLLBACK" => second.as_deref() != Some("TO"),
        "PREPARE" => second.as_deref() == Some("TRANSACTION"),
        _ => false,
    };
    controls.then_some(first)
}

/// The leading words of `sql`, skipping comments before them.
fn code_words(sql: &str) -> impl Iterator<Item = &str> {
    let mut rest = sql.trim_start();
    loop {
        if let Some(after) = rest.strip_prefix("--") {
            rest = after
                .split_once('\n')
                .map_or("", |(_, tail)| tail)
                .trim_start();
        } else if let Some(after) = rest.strip_prefix("/*") {
            rest = after
                .split_once("*/")
                .map_or("", |(_, tail)| tail)
                .trim_start();
        } else {
            break;
        }
    }
    rest.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
}

/// One entry of the `--json` report's `results`.
fn outcome_json(sql: &str, outcome: &Outcome) -> Value {
    match outcome {
        Outcome::Rows { columns, rows } => json!({
            "sql": sql,
            "columns": columns.iter().map(|(name, ty)| json!({ "name": name, "type": ty })).collect::<Vec<_>>(),
            "rows": rows,
            "row_count": rows.len(),
        }),
        Outcome::Affected(count) => json!({ "sql": sql, "rows_affected": count }),
    }
}

/// A result set as an aligned table: header, rule, rows, row count. `NULL`
/// shows as an empty cell.
fn print_table(columns: &[(String, String)], rows: &[Vec<Option<String>>]) {
    let cell = |row: &[Option<String>], index: usize| -> String {
        row.get(index).cloned().flatten().unwrap_or_default()
    };
    let widths: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            rows.iter()
                .map(|row| cell(row, index).chars().count())
                .chain([name.chars().count()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |cells: Vec<String>| -> String {
        cells
            .iter()
            .zip(&widths)
            .map(|(text, width)| format!("{text:<width$}"))
            .collect::<Vec<_>>()
            .join(" | ")
            .trim_end()
            .to_owned()
    };
    println!(
        "{}",
        line(columns.iter().map(|(name, _)| name.clone()).collect())
    );
    println!(
        "{}",
        widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("-+-")
    );
    for row in rows {
        println!(
            "{}",
            line((0..columns.len()).map(|index| cell(row, index)).collect())
        );
    }
    let noun = if rows.len() == 1 { "row" } else { "rows" };
    println!("({} {noun})", rows.len());
}

#[cfg(test)]
mod tests {
    use super::transaction_control;

    #[test]
    fn transaction_control_is_recognized() {
        for sql in [
            "COMMIT",
            "begin",
            "START TRANSACTION",
            "END",
            "ABORT",
            "ROLLBACK",
            "-- note\nCOMMIT",
            "/* x */ rollback work",
            "PREPARE TRANSACTION 'x'",
        ] {
            assert!(transaction_control(sql).is_some(), "{sql}");
        }
    }

    #[test]
    fn statements_inside_the_transaction_are_allowed() {
        for sql in [
            "SELECT 1",
            "ROLLBACK TO SAVEPOINT a",
            "SAVEPOINT a",
            "RELEASE SAVEPOINT a",
            "PREPARE q AS SELECT 1",
            "INSERT INTO commits VALUES (1)",
            "beginning",
        ] {
            assert!(transaction_control(sql).is_none(), "{sql}");
        }
    }
}
