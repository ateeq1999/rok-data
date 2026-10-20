//! A projection: a column, an aggregate, a cast or raw SQL.

use std::fmt;
use std::marker::PhantomData;

use crate::sql::Sql;
use crate::{Model, Value};

use super::{Column, Cond, Direction, Expr, Order, OrderTarget, Term};

/// A value expression of model `M` that can be selected: a column, an
/// aggregate (`User::AGE.avg()`), a cast or raw SQL.
///
/// Use projections with [`Select::select`](crate::Select::select) to fetch
/// tuples or custom row types instead of whole models, and compare them to
/// build `HAVING` conditions:
///
/// ```ignore
/// let per_role: Vec<(String, i64)> = User::query()
///     .group_by(User::ROLE)
///     .having(Projection::count_all().gt(10))
///     .select((User::ROLE, Projection::count_all()))
///     .fetch_all(&db)
///     .await?;
/// ```
pub struct Projection<M> {
    pub(crate) term: Term,
    pub(crate) alias: Option<&'static str>,
    _model: PhantomData<fn() -> M>,
}

impl<M> Projection<M> {
    fn new(term: Term) -> Self {
        Self {
            term,
            alias: None,
            _model: PhantomData,
        }
    }

    pub(crate) fn func(
        func: &'static str,
        arg: Option<(&'static str, &'static str)>,
        distinct: bool,
    ) -> Self {
        Self::new(Term::Func {
            func,
            arg,
            distinct,
        })
    }

    pub(crate) fn retag<N>(self) -> Projection<N> {
        Projection {
            term: self.term,
            alias: self.alias,
            _model: PhantomData,
        }
    }

    /// `COUNT(*)`
    pub fn count_all() -> Self {
        Self::func("COUNT", None, false)
    }

    /// Raw SQL where each `?` is bound to the next parameter.
    pub fn raw<V: Into<Value>>(
        sql: impl Into<String>,
        params: impl IntoIterator<Item = V>,
    ) -> Self {
        Self::new(Term::Raw(
            sql.into(),
            params.into_iter().map(Into::into).collect(),
        ))
    }

    /// `CAST(expr AS sql_type)`, e.g. `.cast("BIGINT")` or
    /// `.cast("DOUBLE PRECISION")` to decode a `NUMERIC` aggregate into
    /// `i64`/`f64`.
    pub fn cast(self, sql_type: &'static str) -> Self {
        Self {
            term: Term::Cast(Box::new(self.term), sql_type),
            ..self
        }
    }

    /// Name the selected value (`expr AS alias`), for decoding into a struct
    /// that derives `sqlx::FromRow`.
    pub fn alias(mut self, alias: &'static str) -> Self {
        self.alias = Some(alias);
        self
    }

    fn cmp(self, op: &'static str, value: impl Into<Value>) -> Expr<M> {
        Expr::new(Cond::Cmp {
            term: self.term,
            op,
            value: value.into(),
        })
    }

    /// `expr = value`
    pub fn eq(self, value: impl Into<Value>) -> Expr<M> {
        self.cmp("=", value)
    }

    /// `expr <> value`
    pub fn ne(self, value: impl Into<Value>) -> Expr<M> {
        self.cmp("<>", value)
    }

    /// `expr > value`
    pub fn gt(self, value: impl Into<Value>) -> Expr<M> {
        self.cmp(">", value)
    }

    /// `expr >= value`
    pub fn gte(self, value: impl Into<Value>) -> Expr<M> {
        self.cmp(">=", value)
    }

    /// `expr < value`
    pub fn lt(self, value: impl Into<Value>) -> Expr<M> {
        self.cmp("<", value)
    }

    /// `expr <= value`
    pub fn lte(self, value: impl Into<Value>) -> Expr<M> {
        self.cmp("<=", value)
    }

    /// Ascending ordering by this expression, e.g. a search rank.
    pub fn asc(self) -> Order<M> {
        Order::with_target(OrderTarget::Term(self.term), Direction::Asc)
    }

    /// Descending ordering by this expression.
    pub fn desc(self) -> Order<M> {
        Order::with_target(OrderTarget::Term(self.term), Direction::Desc)
    }

    pub(crate) fn write(&self, sql: &mut Sql) {
        self.term.write(sql);
        if let Some(alias) = self.alias {
            sql.push(" AS ").push_ident(alias);
        }
    }
}

impl<M: Model> From<Column<M>> for Projection<M> {
    fn from(column: Column<M>) -> Self {
        Self::new(column.term())
    }
}

impl<M> Clone for Projection<M> {
    fn clone(&self) -> Self {
        Self {
            term: self.term.clone(),
            alias: self.alias,
            _model: PhantomData,
        }
    }
}

impl<M> fmt::Debug for Projection<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sql = Sql::new();
        self.write(&mut sql);
        f.debug_tuple("Projection").field(&sql.as_str()).finish()
    }
}
/// A list of [`Projection`]s: a single column or projection, a tuple of up
/// to 12 of them, an array or a `Vec`.
pub trait IntoProjections<M> {
    /// Convert into the list of selected expressions.
    fn into_projections(self) -> Vec<Projection<M>>;
}

impl<M: Model> IntoProjections<M> for Column<M> {
    fn into_projections(self) -> Vec<Projection<M>> {
        vec![self.into()]
    }
}

impl<M> IntoProjections<M> for Projection<M> {
    fn into_projections(self) -> Vec<Projection<M>> {
        vec![self]
    }
}

impl<M, P: Into<Projection<M>>> IntoProjections<M> for Vec<P> {
    fn into_projections(self) -> Vec<Projection<M>> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<M, P: Into<Projection<M>>, const N: usize> IntoProjections<M> for [P; N] {
    fn into_projections(self) -> Vec<Projection<M>> {
        self.into_iter().map(Into::into).collect()
    }
}
