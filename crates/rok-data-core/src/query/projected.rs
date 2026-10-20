//! A `SELECT` of specific columns or aggregates.
use std::fmt;

use futures_core::stream::BoxStream;
use sqlx::postgres::PgRow;
use sqlx::{Decode, FromRow, Postgres, Type};

use crate::exec;
use crate::expr::Projection;
use crate::sql::Sql;
use crate::{Error, Executor, Model, Result};

use super::Select;

/// A `SELECT` of specific columns or aggregates, created with
/// [`Select::select`]. Decode rows into tuples or any `sqlx::FromRow` type.
pub struct Projected<M> {
    pub(crate) select: Select<M>,
    pub(crate) items: Vec<Projection<M>>,
}

impl<M: Model> Projected<M> {
    /// Render the `SELECT` statement.
    pub fn to_sql(&self) -> Sql {
        let mut sql = self.select.new_sql();
        self.write_into(&mut sql);
        sql
    }

    pub(crate) fn write_into(&self, sql: &mut Sql) {
        sql.push("SELECT ");
        sql.push_list(&self.items, ", ", |sql, p| p.write(sql));
        self.select.write_tail(sql);
    }

    /// Fetch every row, decoded into `T` (a tuple or a `FromRow` type).
    pub async fn fetch_all<'e, T, E>(self, executor: E) -> Result<Vec<T>>
    where
        T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
        E: Executor<'e>,
    {
        exec::fetch_all(executor, &self.to_sql(), &[], self.select.use_replica()).await
    }

    /// Fetch the first row, or `None`.
    pub async fn fetch_optional<'e, T, E>(self, executor: E) -> Result<Option<T>>
    where
        T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
        E: Executor<'e>,
    {
        let projected = Projected {
            select: self.select.limit(1),
            items: self.items,
        };
        let replica = projected.select.use_replica();
        exec::fetch_optional(executor, &projected.to_sql(), &[], replica).await
    }

    /// Fetch the first row, failing with [`Error::NotFound`].
    pub async fn fetch_one<'e, T, E>(self, executor: E) -> Result<T>
    where
        T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
        E: Executor<'e>,
    {
        self.fetch_optional(executor)
            .await?
            .ok_or_else(|| Error::not_found::<M>(None))
    }

    /// Fetch the first column of the first row.
    pub async fn scalar<'e, T, E>(self, executor: E) -> Result<T>
    where
        T: Type<Postgres> + for<'r> Decode<'r, Postgres> + Send + Unpin,
        E: Executor<'e>,
    {
        exec::fetch_scalar(executor, &self.to_sql(), self.select.use_replica()).await
    }

    /// Stream rows one at a time.
    pub fn stream<'e, T, E>(self, executor: E) -> BoxStream<'e, Result<T>>
    where
        T: for<'r> FromRow<'r, PgRow> + Send + Unpin + 'e,
        E: Executor<'e> + 'e,
    {
        let replica = self.select.use_replica();
        exec::stream(executor, self.to_sql(), replica)
    }
}

impl<M> Clone for Projected<M> {
    fn clone(&self) -> Self {
        Self {
            select: self.select.clone(),
            items: self.items.clone(),
        }
    }
}

impl<M: Model> fmt::Debug for Projected<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Projected").field(&self.to_sql()).finish()
    }
}
