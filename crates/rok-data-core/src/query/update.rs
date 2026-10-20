//! The partial update builder.
use std::fmt;
use std::marker::PhantomData;

use crate::exec;
use crate::expr::Cond;
use crate::model::push_returning;
use crate::sql::Sql;
use crate::{Column, Error, Executor, Expr, Model, Result, Value};

use super::conflict::quoted;
use super::{Select, Trashed, implicit, push_where};

/// A bulk `UPDATE` over model `M`, created with [`Model::update_all`] or
/// [`Select::update`].
///
/// Managed columns are maintained automatically unless you set them
/// yourself: `updated_at = now()` for models with timestamps and
/// `version = version + 1` for models with optimistic locking. Soft-deleted
/// rows are skipped unless the query used `with_trashed`/`only_trashed`.
pub struct Update<M> {
    pub(crate) filters: Vec<Cond>,
    pub(crate) sets: Vec<(&'static str, Assign)>,
    pub(crate) trashed: Trashed,
    pub(crate) scoped: bool,
    pub(crate) all_tenants: bool,
    pub(crate) _model: PhantomData<fn() -> M>,
}

#[derive(Debug, Clone)]
pub(crate) enum Assign {
    Value(Value),
    Raw(String, Vec<Value>),
}

impl<M: Model> Update<M> {
    pub(crate) fn new() -> Self {
        Self {
            filters: Vec::new(),
            sets: Vec::new(),
            trashed: Trashed::Exclude,
            scoped: true,
            all_tenants: false,
            _model: PhantomData,
        }
    }

    /// Update every tenant's rows, ignoring the current tenant scope.
    pub fn all_tenants(mut self) -> Self {
        self.all_tenants = true;
        self
    }

    /// Ignore the model's default scope.
    pub fn unscoped(mut self) -> Self {
        self.scoped = false;
        self
    }

    /// Add a `WHERE` condition. Multiple calls are combined with `AND`.
    pub fn filter(mut self, expr: Expr<M>) -> Self {
        self.filters.push(expr.cond);
        self
    }

    /// Also update soft-deleted rows.
    pub fn with_trashed(mut self) -> Self {
        self.trashed = Trashed::Include;
        self
    }

    /// `SET column = value`
    pub fn set(mut self, column: Column<M>, value: impl Into<Value>) -> Self {
        self.sets.push((column.name(), Assign::Value(value.into())));
        self
    }

    /// `SET column = <raw sql>`, binding each `?` to the next parameter.
    ///
    /// ```ignore
    /// Post::update_all().set_raw(Post::PUBLISHED_AT, "now()", [] as [i32; 0])
    /// ```
    pub fn set_raw<V: Into<Value>>(
        mut self,
        column: Column<M>,
        sql: impl Into<String>,
        params: impl IntoIterator<Item = V>,
    ) -> Self {
        let params = params.into_iter().map(Into::into).collect();
        self.sets
            .push((column.name(), Assign::Raw(sql.into(), params)));
        self
    }

    /// `SET column = column + by`
    pub fn increment(self, column: Column<M>, by: impl Into<Value>) -> Self {
        let raw = format!("{} + ?", quoted(column.name()));
        self.set_raw(column, raw, [by.into()])
    }

    /// `SET column = column - by`
    pub fn decrement(self, column: Column<M>, by: impl Into<Value>) -> Self {
        let raw = format!("{} - ?", quoted(column.name()));
        self.set_raw(column, raw, [by.into()])
    }

    pub(crate) fn has_sets(&self) -> bool {
        !self.sets.is_empty()
    }

    pub(crate) fn into_select(self) -> Select<M> {
        Select {
            filters: self.filters,
            trashed: self.trashed,
            scoped: self.scoped,
            all_tenants: self.all_tenants,
            ..Select::new()
        }
    }

    fn is_set(&self, column: &str) -> bool {
        self.sets.iter().any(|(c, _)| *c == column)
    }

    /// Render the `UPDATE` statement.
    pub fn to_sql(&self) -> Sql {
        let mut sql = Sql::new();
        self.write_into(&mut sql);
        sql
    }

    pub(crate) fn write_into(&self, sql: &mut Sql) {
        sql.push("UPDATE ").push_ident(M::TABLE).push(" SET ");
        sql.push_list(&self.sets, ", ", |sql, (column, assign)| {
            sql.push_ident(column).push(" = ");
            match assign {
                Assign::Value(v) => sql.bind(v.clone()),
                Assign::Raw(raw, params) => sql.push_raw(raw, params),
            };
        });
        if let Some(updated_at) = M::UPDATED_AT_COLUMN
            && !self.is_set(updated_at)
        {
            sql.push(", ").push_ident(updated_at).push(" = now()");
        }
        if let Some(version) = M::VERSION_COLUMN
            && !self.is_set(version)
        {
            sql.push(", ")
                .push_ident(version)
                .push(" = ")
                .push_ident(version)
                .push(" + 1");
        }
        let implicit = implicit::<M>(self.trashed, self.scoped, self.all_tenants);
        push_where(sql, " WHERE ", self.filters.iter().chain(&implicit));
    }

    fn check(&self) -> Result<()> {
        if let Some(tenant) = M::TENANT_COLUMN
            && self.is_set(tenant)
            && crate::tenant::current().is_some()
        {
            return Err(Error::InvalidQuery(format!(
                "can't change the tenant column `{tenant}` inside a tenant scope"
            )));
        }
        if self.sets.is_empty() {
            return Err(Error::InvalidQuery(format!(
                "UPDATE on `{}` has no columns to set",
                M::TABLE
            )));
        }
        Ok(())
    }

    /// Run the update and return the number of affected rows.
    pub async fn exec<'e, E: Executor<'e>>(self, executor: E) -> Result<u64> {
        self.check()?;
        exec::execute(executor, &self.to_sql(), &[M::TABLE]).await
    }

    /// Run the update and return the updated rows.
    pub async fn returning<'e, E: Executor<'e>>(self, executor: E) -> Result<Vec<M>> {
        self.check()?;
        let mut sql = self.to_sql();
        push_returning::<M>(&mut sql);
        exec::fetch_all(executor, &sql, &[M::TABLE], false).await
    }

    /// Run the update and return the stored first row, or `None` when the
    /// `WHERE` matched nothing: no row had that key (with a version filter,
    /// also when its version no longer matches).
    pub async fn returning_one<'e, E: Executor<'e>>(self, executor: E) -> Result<Option<M>> {
        let mut sql = self.to_sql();
        push_returning::<M>(&mut sql);
        exec::fetch_optional(executor, &sql, &[M::TABLE], false).await
    }
}

impl<M> Clone for Update<M> {
    fn clone(&self) -> Self {
        Self {
            filters: self.filters.clone(),
            sets: self.sets.clone(),
            trashed: self.trashed,
            scoped: self.scoped,
            all_tenants: self.all_tenants,
            _model: PhantomData,
        }
    }
}

impl<M: Model> fmt::Debug for Update<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Update").field(&self.to_sql()).finish()
    }
}
