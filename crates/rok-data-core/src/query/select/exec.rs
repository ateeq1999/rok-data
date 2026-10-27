//! The terminal methods that run a `Select`: fetching, paginating, deleting.
use futures_core::stream::BoxStream;
use sqlx::{Decode, Postgres, Type};

use crate::exec::{self};
use crate::expr::Cond;
use crate::model::{Model, push_columns};
use crate::relation::IntoInclude;
use crate::sql::Sql;
use crate::{Column, Error, Executor, ModelCollection, Result, Value};

use super::Select;
use crate::query::WithSelect;

impl<M: Model> Select<M> {
    /// Loads a relation beside the rows: the batched follow-up query runs
    /// once for the whole match (defeating N+1), and terminals return a
    /// [`Loaded`](crate::relation::Loaded) carrying the rows and their
    /// loaded relations.
    ///
    /// Accepted as the include: a relation constant (`User::POSTS`), or
    /// an [`Include`](crate::relation::Include) with filters, ordering,
    /// limit and offset on the batch.
    ///
    /// ```ignore
    /// let view = User::filter(User::ACTIVE.eq(true))
    ///     .with(User::POSTS)
    ///     .all(&db)
    ///     .await?;
    /// for user in view.rows() {
    ///     let posts = view.try_get(User::POSTS, user);
    /// }
    /// ```
    pub fn with<I: IntoInclude<M>>(self, include: I) -> WithSelect<M> {
        WithSelect {
            select: self,
            includes: vec![include.into_packed()],
        }
    }

    /// Fetch every matching row.
    pub async fn all<'e, E: Executor<'e>>(self, executor: E) -> Result<Vec<M>> {
        exec::fetch_all(executor, &self.to_sql(), &[], self.use_replica()).await
    }

    /// Fetch every matching row as a [`ModelCollection`]. The collection verbs (`find`,
    /// `load`, `fresh`, `to_query`, …) work on it at once, and through
    /// `Deref` it is still a `Vec`; [`all`](Self::all) returns the plain
    /// `Vec`.
    pub async fn get<'e, E: Executor<'e>>(self, executor: E) -> Result<ModelCollection<M>> {
        Ok(self.all(executor).await?.into())
    }

    /// Fetch the first matching row, or `None`.
    pub async fn first<'e, E: Executor<'e>>(self, executor: E) -> Result<Option<M>> {
        let replica = self.use_replica();
        exec::fetch_optional(executor, &self.limit(1).to_sql(), &[], replica).await
    }

    /// Fetch the first matching row, failing with [`Error::NotFound`].
    pub async fn one<'e, E: Executor<'e>>(self, executor: E) -> Result<M> {
        self.first(executor)
            .await?
            .ok_or_else(|| Error::not_found::<M>(None))
    }

    /// Stream matching rows one at a time instead of loading them all into
    /// memory. Consume it with `futures::TryStreamExt` (re-exported in the
    /// rok-data prelude):
    ///
    /// ```ignore
    /// let mut users = User::query().stream(&db);
    /// while let Some(user) = users.try_next().await? {
    ///     // …
    /// }
    /// ```
    pub fn stream<'e, E: Executor<'e> + 'e>(self, executor: E) -> BoxStream<'e, Result<M>> {
        let replica = self.use_replica();
        exec::stream(executor, self.to_sql(), replica)
    }

    /// Count the matching rows (ignores ordering, limit and offset).
    pub async fn count<'e, E: Executor<'e>>(self, executor: E) -> Result<i64> {
        exec::fetch_scalar(executor, &self.count_sql(), self.use_replica()).await
    }

    /// `true` if at least one row matches.
    pub async fn exists<'e, E: Executor<'e>>(self, executor: E) -> Result<bool> {
        exec::fetch_scalar(executor, &self.exists_sql(), self.use_replica()).await
    }

    /// `SUM(column)` over the matching rows as `BIGINT` (`None` when no
    /// rows match). Use [`select`](Self::select) with
    /// [`Projection::cast`](crate::Projection::cast) for other result types.
    pub async fn sum<'e, E: Executor<'e>>(
        self,
        executor: E,
        column: Column<M>,
    ) -> Result<Option<i64>> {
        let sql = self.aggregate_sql(column.sum().cast("BIGINT"));
        exec::fetch_scalar(executor, &sql, self.use_replica()).await
    }

    /// `AVG(column)` over the matching rows as `DOUBLE PRECISION`.
    pub async fn avg<'e, E: Executor<'e>>(
        self,
        executor: E,
        column: Column<M>,
    ) -> Result<Option<f64>> {
        let sql = self.aggregate_sql(column.avg().cast("DOUBLE PRECISION"));
        exec::fetch_scalar(executor, &sql, self.use_replica()).await
    }

    /// `MIN(column)` over the matching rows.
    pub async fn min<'e, T, E>(self, executor: E, column: Column<M>) -> Result<Option<T>>
    where
        T: Type<Postgres> + for<'r> Decode<'r, Postgres> + Send + Unpin,
        E: Executor<'e>,
    {
        exec::fetch_scalar(
            executor,
            &self.aggregate_sql(column.min()),
            self.use_replica(),
        )
        .await
    }

    /// `MAX(column)` over the matching rows.
    pub async fn max<'e, T, E>(self, executor: E, column: Column<M>) -> Result<Option<T>>
    where
        T: Type<Postgres> + for<'r> Decode<'r, Postgres> + Send + Unpin,
        E: Executor<'e>,
    {
        exec::fetch_scalar(
            executor,
            &self.aggregate_sql(column.max()),
            self.use_replica(),
        )
        .await
    }

    pub(crate) fn paginate_sql(&self, page: u64, per_page: u64) -> Sql {
        if !self.joins.is_empty() {
            return self.joined_paginate_sql(page, per_page);
        }
        // `count LEFT JOIN LATERAL (page)` always yields at least one row, so
        // the total is known even when the requested page is empty.
        let mut sql = Sql::new();
        sql.push(r#"SELECT c."__rok_total", p.* FROM (SELECT COUNT(*) AS "__rok_total""#);
        self.write_from_where(&mut sql);
        sql.push(r#") c LEFT JOIN LATERAL (SELECT TRUE AS "__rok_present", "#);
        push_columns::<M>(&mut sql);
        // Expression orders are selected so the outer query can sort by them.
        for (i, o) in self.order.iter().enumerate() {
            if o.column().is_none() {
                sql.push(", ");
                o.write_key(&mut sql, "", None);
                sql.push(" AS ").push_ident(&format!("__rok_o{i}"));
            }
        }
        self.write_from_where(&mut sql);
        self.write_order(&mut sql, "");
        let offset = (page - 1).saturating_mul(per_page);
        sql.push(&format!(" LIMIT {per_page} OFFSET {offset}) p ON TRUE"));
        self.write_order(&mut sql, "p.");
        sql
    }

    /// [`paginate_sql`](Self::paginate_sql) for queries with joins: every
    /// order key is selected under an alias so the outer query can sort by
    /// it, and multiplying joins are de-duplicated.
    pub(crate) fn joined_paginate_sql(&self, page: u64, per_page: u64) -> Sql {
        let mut sql = self.new_sql();
        sql.push(r#"SELECT c."__rok_total", p.* FROM ("#);
        self.write_count_head(&mut sql);
        sql.push(r#" AS "__rok_total""#);
        self.write_from_where(&mut sql);
        sql.push(") c LEFT JOIN LATERAL (");
        let offset = (page - 1).saturating_mul(per_page);
        let head = r#"TRUE AS "__rok_present", "#;
        if self.multiplies() {
            self.write_distinct_rows(&mut sql, head, Some(per_page), Some(offset));
        } else {
            sql.push("SELECT ").push(head);
            push_columns::<M>(&mut sql);
            self.write_order_aliases(&mut sql);
            self.write_from_where(&mut sql);
            self.write_order(&mut sql, "");
            sql.push(&format!(" LIMIT {per_page} OFFSET {offset}"));
        }
        sql.push(") p ON TRUE");
        self.write_alias_order(&mut sql, "p.");
        sql
    }

    /// Delete every matching row and return how many were deleted.
    ///
    /// For models with soft deletes this sets `deleted_at = now()` instead;
    /// use [`force_delete`](Self::force_delete) to remove rows for good.
    pub async fn delete<'e, E: Executor<'e>>(self, executor: E) -> Result<u64> {
        exec::execute(executor, &self.delete_sql(), &[M::TABLE]).await
    }

    /// Render the statement [`delete`](Self::delete) would run.
    pub fn delete_sql(&self) -> Sql {
        match M::DELETED_AT_COLUMN {
            Some(deleted_at) => self
                .clone()
                .update()
                .set_raw(Column::new(deleted_at), "now()", [] as [Value; 0])
                .to_sql(),
            None => self.force_delete_sql(),
        }
    }

    /// `DELETE` every matching row, even for models with soft deletes.
    /// Soft-deleted rows are only matched with
    /// [`with_trashed`](Self::with_trashed) or [`only_trashed`](Self::only_trashed).
    pub async fn force_delete<'e, E: Executor<'e>>(self, executor: E) -> Result<u64> {
        exec::execute(executor, &self.force_delete_sql(), &[M::TABLE]).await
    }

    pub(crate) fn force_delete_sql(&self) -> Sql {
        let mut sql = Sql::new();
        sql.push("DELETE");
        self.write_from_where(&mut sql);
        sql
    }

    /// Restore every matching soft-deleted row (`deleted_at = NULL`) and
    /// return how many were restored.
    pub async fn restore<'e, E: Executor<'e>>(self, executor: E) -> Result<u64> {
        let Some(deleted_at) = M::DELETED_AT_COLUMN else {
            return Err(Error::InvalidQuery(format!(
                "`{}` has no soft-delete column",
                M::TABLE
            )));
        };
        self.only_trashed()
            .update()
            .set_raw(Column::new(deleted_at), "NULL", [] as [Value; 0])
            .exec(executor)
            .await
    }

    /// Every table this query reads: the root and joined tables.
    pub(crate) fn tables(&self) -> Vec<&'static str> {
        std::iter::once(M::TABLE)
            .chain(self.joins.iter().map(|j| j.table))
            .collect()
    }

    /// A fresh `SELECT` of `M` restricted to the root rows this (joined)
    /// query matches: `pk IN (SELECT root.pk FROM … JOIN … WHERE …)`.
    /// Used for bulk writes through joins.
    pub(crate) fn key_subselect(&self) -> Select<M> {
        let inner = self.clone();
        let render = crate::expr::Render::new(move |sql: &mut Sql| {
            let previous = std::mem::replace(&mut sql.qualify, true);
            sql.push("SELECT ");
            Self::write_primary_keys(sql);
            inner.write_from_where(sql);
            sql.qualify = previous;
        });
        let kind = if M::PRIMARY_KEYS.len() == 1 {
            crate::expr::SubKind::In {
                table: M::TABLE,
                column: M::PRIMARY_KEY,
                negated: false,
            }
        } else {
            crate::expr::SubKind::InRow {
                table: M::TABLE,
                columns: M::PRIMARY_KEYS,
            }
        };
        Select {
            filters: vec![Cond::Sub { kind, render }],
            trashed: self.trashed,
            scoped: self.scoped,
            all_tenants: self.all_tenants,
            ..Select::new()
        }
    }
}
