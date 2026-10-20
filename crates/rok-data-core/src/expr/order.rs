//! The `ORDER BY` terms.

use std::fmt;
use std::marker::PhantomData;

use crate::Model;
use crate::sql::Sql;

use super::{Column, Term};

/// Sort direction for `ORDER BY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Ascending (`ASC`).
    Asc,
    /// Descending (`DESC`).
    Desc,
}

/// An `ORDER BY` term on model `M`, created with [`Column::asc`] or [`Column::desc`].
pub struct Order<M> {
    pub(crate) target: OrderTarget,
    pub(crate) direction: Direction,
    pub(crate) nulls: Option<bool>,
    _model: PhantomData<fn() -> M>,
}

/// What an `ORDER BY` term sorts by.
#[derive(Debug, Clone)]
pub(crate) enum OrderTarget {
    Column {
        table: &'static str,
        name: &'static str,
    },
    Term(Term),
}

impl<M> Order<M> {
    pub(crate) fn new(table: &'static str, column: &'static str, direction: Direction) -> Self {
        Self::with_target(
            OrderTarget::Column {
                table,
                name: column,
            },
            direction,
        )
    }

    pub(crate) fn with_target(target: OrderTarget, direction: Direction) -> Self {
        Self {
            target,
            direction,
            nulls: None,
            _model: PhantomData,
        }
    }

    /// The column, if this orders by a plain column.
    pub(crate) fn column(&self) -> Option<&'static str> {
        match self.target {
            OrderTarget::Column { name, .. } => Some(name),
            OrderTarget::Term(_) => None,
        }
    }

    pub(crate) fn retag<N>(self) -> Order<N> {
        Order {
            target: self.target,
            direction: self.direction,
            nulls: self.nulls,
            _model: PhantomData,
        }
    }

    /// Put `NULL`s first.
    pub fn nulls_first(mut self) -> Self {
        self.nulls = Some(true);
        self
    }

    /// Put `NULL`s last.
    pub fn nulls_last(mut self) -> Self {
        self.nulls = Some(false);
        self
    }

    /// Write the sort key: the column or expression, or `alias` instead of an
    /// expression (used when the expression was selected under that alias).
    pub(crate) fn write_key(&self, sql: &mut Sql, prefix: &str, alias: Option<&str>) {
        match (&self.target, alias) {
            (OrderTarget::Column { table, name }, _) => {
                if prefix.is_empty() {
                    sql.push_column(table, name);
                } else {
                    sql.push(prefix).push_ident(name);
                }
            }
            (OrderTarget::Term(_), Some(alias)) => {
                sql.push(prefix).push_ident(alias);
            }
            (OrderTarget::Term(term), None) => term.write(sql),
        }
    }

    pub(crate) fn write_direction(&self, sql: &mut Sql) {
        sql.push(match self.direction {
            Direction::Asc => " ASC",
            Direction::Desc => " DESC",
        });
        match self.nulls {
            Some(true) => sql.push(" NULLS FIRST"),
            Some(false) => sql.push(" NULLS LAST"),
            None => sql,
        };
    }

    pub(crate) fn write(&self, sql: &mut Sql) {
        self.write_key(sql, "", None);
        self.write_direction(sql);
    }
}

impl<M> Clone for Order<M> {
    fn clone(&self) -> Self {
        Self {
            target: self.target.clone(),
            direction: self.direction,
            nulls: self.nulls,
            _model: PhantomData,
        }
    }
}

impl<M> fmt::Debug for Order<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sql = Sql::new();
        self.write(&mut sql);
        f.debug_tuple("Order").field(&sql.as_str()).finish()
    }
}

impl<M: Model> From<Column<M>> for Order<M> {
    fn from(column: Column<M>) -> Self {
        column.asc()
    }
}
