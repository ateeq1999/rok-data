//! Reading the schema statements out of a schema file: the tables (`CREATE
//! TABLE`), the types and indexes, the comments, extensions and views, and
//! everything else, which rides as written — an `Extra`.

use sqlparser::ast::{CommentObject, CreateTable, Statement, UserDefinedTypeRepresentation};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::Error;
use crate::ir::{EnumType, Extra, Index, Schema, Table};
use crate::parse::alter::{self, add_table_constraint, build_column, place_column};
use crate::parse::check::{object_name, quote};
use crate::parse::comments::column_comments;
use crate::split::RawStatement;

fn parse_one(sql: &str) -> Option<Statement> {
    let mut statements = Parser::parse_sql(&PostgreSqlDialect {}, sql).ok()?;
    (statements.len() == 1).then(|| statements.remove(0))
}

pub(crate) fn schema_statement(
    stmt: &RawStatement,
    module: &str,
    doc: Vec<String>,
    explicit_down: Option<String>,
    at: &str,
    schema: &mut Schema,
    comments: &mut Vec<(String, Option<String>, Option<String>)>,
) -> Result<(), Error> {
    let parsed = parse_one(&stmt.sql);
    let looks_like_table = stmt
        .sql
        .split_whitespace()
        .take(2)
        .map(str::to_ascii_uppercase)
        .eq(["CREATE", "TABLE"]);
    match parsed {
        Some(Statement::CreateTable(table)) => {
            let table = table_from(&table, module, doc, &stmt.sql, at)?;
            if schema.tables.iter().any(|t| t.name == table.name) {
                return Err(Error::at(
                    at,
                    format!("table `{}` is declared twice", table.name),
                ));
            }
            schema.tables.push(table);
        }
        Some(Statement::CreateType {
            name,
            representation: Some(UserDefinedTypeRepresentation::Enum { labels }),
        }) => {
            schema.enums.push(EnumType {
                name: object_name(&name),
                values: labels.iter().map(|l| l.value.clone()).collect(),
                module: module.to_owned(),
                doc,
            });
        }
        Some(Statement::CreateIndex(index)) => {
            let Some(name) = &index.name else {
                return Err(Error::at(
                    at,
                    "give the index a name (`CREATE INDEX <name> ON ...`)",
                ));
            };
            schema.indexes.push(Index {
                name: object_name(name),
                table: object_name(&index.table_name),
                sql: Statement::CreateIndex(index.clone()).to_string(),
            });
        }
        Some(Statement::Comment {
            object_type,
            object_name: target,
            comment,
            ..
        }) if matches!(object_type, CommentObject::Table | CommentObject::Column) => {
            let target = object_name(&target);
            let (table, column) = match object_type {
                CommentObject::Column => match target.rsplit_once('.') {
                    Some((t, c)) => (t.to_owned(), Some(c.to_owned())),
                    None => return Err(Error::at(at, "expected `COMMENT ON COLUMN table.column`")),
                },
                _ => (target, None),
            };
            let what = if column.is_some() { "COLUMN" } else { "TABLE" };
            comments.push((table, column, comment.clone()));
            schema.extras.push(Extra {
                up: stmt.sql.clone(),
                down: format!("COMMENT ON {what} {} IS NULL", stmt_target(&stmt.sql)),
                before_tables: false,
            });
        }
        Some(Statement::CreateExtension(ext)) => schema.extras.push(Extra {
            up: stmt.sql.clone(),
            down: explicit_down
                .clone()
                .unwrap_or_else(|| format!("DROP EXTENSION IF EXISTS {}", quote(&ext.name.value))),
            before_tables: true,
        }),
        Some(Statement::CreateView(view)) if explicit_down.is_none() => schema.extras.push(Extra {
            up: stmt.sql.clone(),
            down: format!("DROP VIEW IF EXISTS {}", view.name),
            before_tables: false,
        }),
        // An `ALTER TABLE` that folds into the tables needs no `-- rok:down`
        // and rides no `Extra`; a clause the model cannot represent falls
        // through to the extras path below, wording and all.
        Some(Statement::AlterTable(alter_table))
            if alter::apply(schema, &alter_table.name, &alter_table.operations).is_ok() => {}
        None if looks_like_table => {
            return Err(Error::at(at, "couldn't parse this CREATE TABLE statement"));
        }
        _ => {
            let Some(down) = explicit_down else {
                return Err(Error::at(
                    at,
                    "rok can't undo this statement by itself; follow it with \
                     `-- rok:down` and the SQL that undoes it",
                ));
            };
            schema.extras.push(Extra {
                up: stmt.sql.clone(),
                down,
                before_tables: false,
            });
        }
    }
    Ok(())
}

/// `COMMENT ON COLUMN users.email IS '...'` -> `users.email`.
fn stmt_target(sql: &str) -> String {
    let words: Vec<&str> = sql.split_whitespace().collect();
    words.get(3).copied().unwrap_or_default().to_owned()
}

fn table_from(
    table: &CreateTable,
    module: &str,
    doc: Vec<String>,
    sql: &str,
    at: &str,
) -> Result<Table, Error> {
    let name = object_name(&table.name);
    let column_notes = column_comments(sql);
    let mut out = Table {
        name: name.clone(),
        module: module.to_owned(),
        doc,
        columns: Vec::new(),
        primary_key: Vec::new(),
        uniques: Vec::new(),
        foreign_keys: Vec::new(),
        checks: Vec::new(),
    };
    // The columns and the constraints their options imply: the same
    // builders `ALTER TABLE ... ADD COLUMN` goes through.
    for def in &table.columns {
        let built =
            build_column(&out, def, &column_notes).map_err(|reason| Error::at(at, reason))?;
        place_column(&mut out, built);
    }
    let mut unnamed_checks = 0;
    for constraint in &table.constraints {
        add_table_constraint(&mut out, constraint, &mut unnamed_checks)
            .map_err(|reason| Error::at(at, reason))?;
    }
    // Primary key columns are NOT NULL.
    for column in &mut out.columns {
        if out.primary_key.contains(&column.name) {
            column.nullable = false;
        }
    }
    Ok(out)
}
