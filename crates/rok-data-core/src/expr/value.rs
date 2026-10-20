//! A value expression: what a filter compares against.

use std::fmt;
use std::marker::PhantomData;
use std::ops::Not;
use std::sync::Arc;

use crate::sql::Sql;
use crate::{Model, Select, Value};

use super::{Cond, SubKind};

/// A boolean condition on model `M`, used in `WHERE` clauses.
///
/// Combine conditions with [`Expr::and`], [`Expr::or`] and `!` (negation),
/// or build them from raw SQL with [`Expr::raw`].
pub struct Expr<M> {
    pub(crate) cond: Cond,
    _model: PhantomData<fn() -> M>,
}

impl<M> Expr<M> {
    pub(crate) fn new(cond: Cond) -> Self {
        Self {
            cond,
            _model: PhantomData,
        }
    }

    /// A raw SQL condition where each `?` is bound to the next parameter.
    ///
    /// ```ignore
    /// Expr::raw("lower(email) = lower(?)", ["Ann@Example.com"])
    /// ```
    pub fn raw<V: Into<Value>>(
        sql: impl Into<String>,
        params: impl IntoIterator<Item = V>,
    ) -> Self {
        Self::new(Cond::Raw {
            sql: sql.into(),
            params: params.into_iter().map(Into::into).collect(),
        })
    }

    /// A condition that is always true.
    pub fn all() -> Self {
        Self::new(Cond::And(Vec::new()))
    }

    /// A condition that is always false.
    pub fn none() -> Self {
        Self::new(Cond::Or(Vec::new()))
    }

    /// `EXISTS (subquery)`. Combine with [`Column::eq_outer`] to correlate
    /// the subquery with the outer query.
    pub fn exists<N: Model>(subquery: Select<N>) -> Self {
        Self::new(Cond::Sub {
            kind: SubKind::Exists { negated: false },
            render: Render::new(move |sql| subquery.write_exists_body(sql)),
        })
    }

    /// `NOT EXISTS (subquery)`.
    pub fn not_exists<N: Model>(subquery: Select<N>) -> Self {
        Self::new(Cond::Sub {
            kind: SubKind::Exists { negated: true },
            render: Render::new(move |sql| subquery.write_exists_body(sql)),
        })
    }

    /// Both `self` and `other` must hold.
    pub fn and(self, other: Expr<M>) -> Self {
        Self::new(match self.cond {
            Cond::And(mut list) => {
                list.push(other.cond);
                Cond::And(list)
            }
            cond => Cond::And(vec![cond, other.cond]),
        })
    }

    /// Either `self` or `other` must hold.
    pub fn or(self, other: Expr<M>) -> Self {
        Self::new(match self.cond {
            Cond::Or(mut list) if !list.is_empty() => {
                list.push(other.cond);
                Cond::Or(list)
            }
            cond => Cond::Or(vec![cond, other.cond]),
        })
    }

    /// Every condition in `exprs` must hold (`TRUE` when empty).
    pub fn all_of(exprs: impl IntoIterator<Item = Expr<M>>) -> Self {
        Self::new(Cond::And(exprs.into_iter().map(|e| e.cond).collect()))
    }

    /// At least one condition in `exprs` must hold (`FALSE` when empty).
    pub fn any_of(exprs: impl IntoIterator<Item = Expr<M>>) -> Self {
        Self::new(Cond::Or(exprs.into_iter().map(|e| e.cond).collect()))
    }
}

impl<M> Expr<M> {
    /// Re-tag a condition for another model (used by joins, whose
    /// conditions are already table-qualified).
    pub(crate) fn retag<N>(self) -> Expr<N> {
        Expr::new(self.cond)
    }
}

impl<M> Not for Expr<M> {
    type Output = Expr<M>;

    fn not(self) -> Self::Output {
        Self::new(Cond::Not(Box::new(self.cond)))
    }
}

impl<M> Clone for Expr<M> {
    fn clone(&self) -> Self {
        Self::new(self.cond.clone())
    }
}

impl<M> fmt::Debug for Expr<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cond.fmt(f)
    }
}

/// Something that renders to a SQL value expression.
#[derive(Debug, Clone)]
pub(crate) enum Term {
    Column {
        table: &'static str,
        name: &'static str,
    },
    Func {
        func: &'static str,
        arg: Option<(&'static str, &'static str)>,
        distinct: bool,
    },
    Cast(Box<Term>, &'static str),
    Raw(String, Vec<Value>),
}

impl Term {
    pub(crate) fn write(&self, sql: &mut Sql) {
        match self {
            Term::Column { table, name } => {
                sql.push_column(table, name);
            }
            Term::Func {
                func,
                arg,
                distinct,
            } => {
                sql.push(func).push("(");
                if *distinct {
                    sql.push("DISTINCT ");
                }
                match arg {
                    Some((table, column)) => sql.push_column(table, column),
                    None => sql.push("*"),
                };
                sql.push(")");
            }
            Term::Cast(inner, ty) => {
                sql.push("CAST(");
                inner.write(sql);
                sql.push(" AS ").push(ty).push(")");
            }
            Term::Raw(raw, params) => {
                sql.push("(").push_raw(raw, params).push(")");
            }
        }
    }
}

/// Renders a subquery into the enclosing statement (sharing its parameter
/// numbering).
#[derive(Clone)]
pub(crate) struct Render(pub(crate) Arc<dyn Fn(&mut Sql) + Send + Sync>);

impl Render {
    pub(crate) fn new(f: impl Fn(&mut Sql) + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }
}

impl fmt::Debug for Render {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sql = Sql::new();
        (self.0)(&mut sql);
        f.write_str(sql.as_str())
    }
}
