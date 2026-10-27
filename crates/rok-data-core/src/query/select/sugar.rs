//! Chain sugar on [`Select`]: one condition over several columns, raw SQL
//! escapes, composing a chain with functions, printing it, and `UNION`.

use crate::expr::Term;
use crate::{Column, Expr, Model, Projection, Value};

use super::super::{Projected, Union};
use super::Select;

impl<M: Model> Select<M> {
    /// Rows where `condition` holds for **any** of `columns`, one call for
    /// a search over several columns:
    ///
    /// ```ignore
    /// let pattern = format!("%{q}%");
    /// User::query().filter_any([User::NAME, User::EMAIL], |c| c.ilike(&pattern))
    /// ```
    ///
    /// No columns matches nothing.
    pub fn filter_any(
        self,
        columns: impl IntoIterator<Item = Column<M>>,
        condition: impl FnMut(Column<M>) -> Expr<M>,
    ) -> Self {
        self.filter(Expr::any_of(columns.into_iter().map(condition)))
    }

    /// Rows where `condition` holds for **all** of `columns`. No columns
    /// matches every row.
    pub fn filter_all(
        self,
        columns: impl IntoIterator<Item = Column<M>>,
        condition: impl FnMut(Column<M>) -> Expr<M>,
    ) -> Self {
        self.filter(Expr::all_of(columns.into_iter().map(condition)))
    }

    /// Rows where `condition` holds for **none** of `columns`. No columns
    /// matches every row.
    pub fn filter_none(
        self,
        columns: impl IntoIterator<Item = Column<M>>,
        condition: impl FnMut(Column<M>) -> Expr<M>,
    ) -> Self {
        self.filter(!Expr::any_of(columns.into_iter().map(condition)))
    }

    /// A raw `WHERE` condition, each `?` bound to the next of `params`:
    /// `.filter_raw("lower(email) = ?", [email])`.
    pub fn filter_raw<V: Into<Value>>(
        self,
        sql: impl Into<String>,
        params: impl IntoIterator<Item = V>,
    ) -> Self {
        self.filter(Expr::raw(sql, params))
    }

    /// A raw `HAVING` condition, each `?` bound to the next of `params`.
    pub fn having_raw<V: Into<Value>>(
        self,
        sql: impl Into<String>,
        params: impl IntoIterator<Item = V>,
    ) -> Self {
        self.having(Expr::raw(sql, params))
    }

    /// Order by a raw SQL expression, ascending; for descending, pass
    /// `Projection::raw(sql, params).desc()` to [`order_by`](Self::order_by).
    pub fn order_by_raw<V: Into<Value>>(
        self,
        sql: impl Into<String>,
        params: impl IntoIterator<Item = V>,
    ) -> Self {
        self.order_by(Projection::raw(sql, params).asc())
    }

    /// Group by a raw SQL expression, each `?` bound to the next of
    /// `params`: `.group_by_raw("date_trunc(?, created_at)", ["day"])`.
    pub fn group_by_raw<V: Into<Value>>(
        mut self,
        sql: impl Into<String>,
        params: impl IntoIterator<Item = V>,
    ) -> Self {
        let params = params.into_iter().map(Into::into).collect();
        self.group_by.push(Term::Raw(sql.into(), params));
        self
    }

    /// Select one raw SQL expression instead of whole models, each `?`
    /// bound to the next of `params`; fetch it with the
    /// [`Projected`] terminals.
    pub fn select_raw<V: Into<Value>>(
        self,
        sql: impl Into<String>,
        params: impl IntoIterator<Item = V>,
    ) -> Projected<M> {
        self.select(Projection::raw(sql, params))
    }

    /// Hand the chain to `f` and return what it returns: a reusable step
    /// that may end the chain, or turn it into something else.
    ///
    /// ```ignore
    /// fn recent(q: Select<Post>) -> Select<Post> { q.order_by(Post::ID.desc()).limit(10) }
    /// let posts = Post::query().pipe(recent).all(&db).await?;
    /// ```
    pub fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }

    /// Look at the chain without changing it (to log it, say) and go on.
    pub fn tap(self, f: impl FnOnce(&Self)) -> Self {
        f(&self);
        self
    }

    /// Print the statement and its parameters to stderr, then go on: a
    /// development aid, like `dbg!`, for the chain.
    pub fn debug(self) -> Self {
        let sql = self.to_sql();
        eprintln!("{} -- {:?}", sql.as_str(), sql.params());
        self
    }

    /// `this UNION other`: the rows of both, without duplicates. Order,
    /// limit and offset on the result go on the returned [`Union`].
    pub fn union(self, other: Select<M>) -> Union<M> {
        Union::new(self).union(other)
    }

    /// `this UNION ALL other`: the rows of both, duplicates kept.
    pub fn union_all(self, other: Select<M>) -> Union<M> {
        Union::new(self).union_all(other)
    }
}
