//! The conditions a filter is built from.

use crate::sql::Sql;
use crate::value::Value;

use super::{Render, Term};

#[derive(Debug, Clone)]
pub(crate) enum SubKind {
    /// `(a, b) IN (subquery)` for composite keys.
    InRow {
        table: &'static str,
        columns: &'static [&'static str],
    },
    In {
        table: &'static str,
        column: &'static str,
        negated: bool,
    },
    Exists {
        negated: bool,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum Cond {
    Sub {
        kind: SubKind,
        render: Render,
    },
    Columns {
        left: (&'static str, &'static str),
        op: &'static str,
        right: (&'static str, &'static str),
    },
    Cmp {
        term: Term,
        op: &'static str,
        value: Value,
    },
    In {
        table: &'static str,
        column: &'static str,
        values: Vec<Value>,
        negated: bool,
    },
    Between {
        table: &'static str,
        column: &'static str,
        low: Value,
        high: Value,
    },
    Null {
        table: &'static str,
        column: &'static str,
        negated: bool,
    },
    Raw {
        sql: String,
        params: Vec<Value>,
    },
    And(Vec<Cond>),
    Or(Vec<Cond>),
    Not(Box<Cond>),
}
impl Cond {
    pub(crate) fn write(&self, sql: &mut Sql) {
        match self {
            Cond::Sub { kind, render } => {
                match kind {
                    SubKind::InRow { table, columns } => {
                        sql.push("(")
                            .push_list(columns.iter(), ", ", |sql, c| {
                                sql.push_column(table, c);
                            })
                            .push(") IN (");
                    }
                    SubKind::In {
                        table,
                        column,
                        negated,
                    } => {
                        sql.push_column(table, column).push(if *negated {
                            " NOT IN ("
                        } else {
                            " IN ("
                        });
                    }
                    SubKind::Exists { negated } => {
                        sql.push(if *negated { "NOT EXISTS (" } else { "EXISTS (" });
                    }
                }
                (render.0)(sql);
                sql.push(")");
            }
            Cond::Columns { left, op, right } => {
                sql.push(&crate::sql::qualified(left.0, left.1));
                sql.push(" ").push(op).push(" ");
                sql.push(&crate::sql::qualified(right.0, right.1));
            }
            Cond::Cmp { term, op, value } => {
                term.write(sql);
                sql.push(" ").push(op).push(" ");
                sql.bind(value.clone());
            }
            Cond::In {
                table,
                column,
                values,
                negated,
            } => {
                if values.is_empty() {
                    sql.push(if *negated { "TRUE" } else { "FALSE" });
                    return;
                }
                sql.push_column(table, column)
                    .push(if *negated { " NOT IN (" } else { " IN (" });
                sql.push_list(values, ", ", |sql, v| {
                    sql.bind(v.clone());
                });
                sql.push(")");
            }
            Cond::Between {
                table,
                column,
                low,
                high,
            } => {
                sql.push_column(table, column).push(" BETWEEN ");
                sql.bind(low.clone()).push(" AND ").bind(high.clone());
            }
            Cond::Null {
                table,
                column,
                negated,
            } => {
                sql.push_column(table, column).push(if *negated {
                    " IS NOT NULL"
                } else {
                    " IS NULL"
                });
            }
            Cond::Raw { sql: raw, params } => {
                sql.push("(").push_raw(raw, params).push(")");
            }
            Cond::And(list) => Self::write_group(sql, list, " AND ", "TRUE"),
            Cond::Or(list) => Self::write_group(sql, list, " OR ", "FALSE"),
            Cond::Not(inner) => {
                sql.push("NOT (");
                inner.write(sql);
                sql.push(")");
            }
        }
    }

    fn write_group(sql: &mut Sql, list: &[Cond], sep: &str, empty: &str) {
        match list {
            [] => {
                sql.push(empty);
            }
            [single] => single.write(sql),
            many => {
                sql.push("(");
                sql.push_list(many, sep, |sql, c| c.write(sql));
                sql.push(")");
            }
        }
    }
}
