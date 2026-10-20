//! Reading the `CREATE TABLE` statements out of a schema file.

use sqlparser::ast::{
    ColumnOption, CommentObject, CreateTable, GeneratedAs, Statement, TableConstraint,
    UserDefinedTypeRepresentation,
};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::ir::{Check, Column, EnumType, Extra, ForeignKey, Index, Schema, Table, Unique};
use crate::parse::check::{constraint_name, expr_ident, ident, object_name, quote};
use crate::parse::comments::column_comments;
use crate::split::RawStatement;
use crate::{Error, naming, types};

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
    let short = naming::unqualified(&name).to_owned();
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
    for def in &table.columns {
        let column_name = ident(&def.name);
        let mut column = Column {
            name: column_name.clone(),
            sql_type: types::canonical(&def.data_type.to_string()),
            nullable: true,
            default: None,
            generated: None,
            identity: None,
            tenant: false,
            doc: Vec::new(),
        };
        for option in &def.options {
            match &option.option {
                ColumnOption::NotNull => column.nullable = false,
                ColumnOption::Null => column.nullable = true,
                ColumnOption::Default(expr) => column.default = Some(expr.to_string()),
                ColumnOption::PrimaryKey(_) => {
                    if !out.primary_key.is_empty() {
                        return Err(Error::at(
                            at,
                            format!("table `{name}` has two primary keys"),
                        ));
                    }
                    out.primary_key = vec![column_name.clone()];
                }
                ColumnOption::Unique(_) => out.uniques.push(Unique {
                    name: format!("{short}_{column_name}_key"),
                    columns: vec![column_name.clone()],
                }),
                ColumnOption::ForeignKey(fk) => out.foreign_keys.push(ForeignKey {
                    name: constraint_name(fk.name.as_ref(), || {
                        format!("{short}_{column_name}_fkey")
                    }),
                    columns: vec![column_name.clone()],
                    ref_table: object_name(&fk.foreign_table),
                    ref_columns: fk.referred_columns.iter().map(ident).collect(),
                    on_delete: fk.on_delete.as_ref().map(ToString::to_string),
                    on_update: fk.on_update.as_ref().map(ToString::to_string),
                }),
                ColumnOption::Check(check) => out.checks.push(Check {
                    name: constraint_name(check.name.as_ref(), || {
                        format!("{short}_{column_name}_check")
                    }),
                    expr: check.expr.to_string(),
                }),
                ColumnOption::Generated {
                    generated_as,
                    generation_expr,
                    ..
                } => match generation_expr {
                    Some(expr) => column.generated = Some(expr.to_string()),
                    None => {
                        column.identity = Some(
                            match generated_as {
                                GeneratedAs::Always => "ALWAYS",
                                _ => "BY DEFAULT",
                            }
                            .to_owned(),
                        );
                    }
                },
                other => {
                    return Err(Error::at(
                        at,
                        format!("column option `{other}` on `{column_name}` is not supported"),
                    ));
                }
            }
        }
        if let Some((doc, tenant)) = column_notes
            .iter()
            .find_map(|(c, d, t)| (c.eq_ignore_ascii_case(&column_name)).then(|| (d.clone(), *t)))
        {
            column.doc = doc;
            column.tenant = tenant;
        }
        out.columns.push(column);
    }
    let mut unnamed_checks = 0;
    for constraint in &table.constraints {
        match constraint {
            TableConstraint::PrimaryKey(pk) => {
                if !out.primary_key.is_empty() {
                    return Err(Error::at(
                        at,
                        format!("table `{name}` has two primary keys"),
                    ));
                }
                out.primary_key = pk
                    .columns
                    .iter()
                    .map(|c| expr_ident(&c.column.expr))
                    .collect();
            }
            TableConstraint::Unique(u) => {
                let columns: Vec<String> = u
                    .columns
                    .iter()
                    .map(|c| expr_ident(&c.column.expr))
                    .collect();
                out.uniques.push(Unique {
                    name: constraint_name(u.name.as_ref(), || {
                        format!("{short}_{}_key", columns.join("_"))
                    }),
                    columns,
                });
            }
            TableConstraint::ForeignKey(fk) => {
                let columns: Vec<String> = fk.columns.iter().map(ident).collect();
                out.foreign_keys.push(ForeignKey {
                    name: constraint_name(fk.name.as_ref(), || {
                        format!("{short}_{}_fkey", columns.join("_"))
                    }),
                    columns,
                    ref_table: object_name(&fk.foreign_table),
                    ref_columns: fk.referred_columns.iter().map(ident).collect(),
                    on_delete: fk.on_delete.as_ref().map(ToString::to_string),
                    on_update: fk.on_update.as_ref().map(ToString::to_string),
                });
            }
            TableConstraint::Check(check) => {
                let name = constraint_name(check.name.as_ref(), || {
                    unnamed_checks += 1;
                    match unnamed_checks {
                        1 => format!("{short}_check"),
                        n => format!("{short}_check{}", n - 1),
                    }
                });
                out.checks.push(Check {
                    name,
                    expr: check.expr.to_string(),
                });
            }
            other => {
                return Err(Error::at(
                    at,
                    format!("table constraint `{other}` is not supported"),
                ));
            }
        }
    }
    // Primary key columns are NOT NULL.
    for column in &mut out.columns {
        if out.primary_key.contains(&column.name) {
            column.nullable = false;
        }
    }
    Ok(out)
}
