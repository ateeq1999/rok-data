//! The `ModelWrite` trait: the single-row write operations.
//!
//! Implemented by `#[derive(Model)]` alongside `Model`. Hand-written models
//! that implement `Model` can also implement `ModelWrite` (the trait provides
//! default implementations that delegate to the `writes` module).

use crate::{Column, Error, Executor, Result, Value};

/// The single-row write surface. Every `Model` also implements `ModelWrite`
/// through the derive, so `use rok_data::prelude::*;` brings both into scope.
pub trait ModelWrite: crate::Model + Sized + 'static {
    /// `INSERT` this record (skipping generated columns) and return the stored row.
    async fn insert<'e, E>(&self, executor: E) -> Result<Self>
    where
        E: Executor<'e>,
    {
        crate::model::writes::insert(self, executor)
            .await
            .map_err(crate::constraint::as_field_error::<Self>)
    }

    /// `INSERT` many records in a single statement and return the stored rows.
    async fn insert_all<'e, E>(executor: E, records: &[Self]) -> Result<Vec<Self>>
    where
        E: Executor<'e>,
    {
        Self::insert_many(records)
            .exec(executor)
            .await
            .map_err(crate::constraint::as_field_error::<Self>)
    }

    /// Bulk-load `records` with PostgreSQL's binary `COPY`, typically 5–20×
    /// faster than `INSERT` for large batches. Returns the number of rows
    /// written.
    async fn copy_in<'a>(
        target: impl Into<crate::copy::CopyTarget<'a>>,
        records: &[Self],
    ) -> Result<u64> {
        let target = target.into();
        crate::copy::copy_in(target, records).await
    }

    /// `INSERT` this record, or update every column if its primary key
    /// already exists (`ON CONFLICT (pk) DO UPDATE`).
    async fn upsert<'e, E>(&self, executor: E) -> Result<Self>
    where
        E: Executor<'e>,
    {
        crate::model::writes::upsert(self, executor)
            .await
            .map_err(crate::constraint::as_field_error::<Self>)
    }

    /// `INSERT` this record, or update it if it conflicts on `columns` (a
    /// unique index).
    async fn upsert_on<'e, E>(
        &self,
        executor: E,
        columns: impl IntoIterator<Item = Column<Self>>,
    ) -> Result<Self>
    where
        E: Executor<'e>,
    {
        crate::model::writes::upsert_on(
            self,
            executor,
            columns.into_iter().map(|c| c.name()).collect(),
        )
        .await
        .map_err(crate::constraint::as_field_error::<Self>)
    }

    /// The row holding this record's values of `columns` (a unique key),
    /// inserting this record first when there is none — atomically, in one
    /// statement:
    ///
    /// ```ignore
    /// let tag = Tag { id: 0, slug: "rust".into() }
    ///     .first_or_insert(&db, [Tag::SLUG])
    ///     .await?;
    /// ```
    ///
    /// To update the existing row instead, use [`upsert_on`](Self::upsert_on).
    async fn first_or_insert<'e, E>(
        &self,
        executor: E,
        columns: impl IntoIterator<Item = Column<Self>>,
    ) -> Result<Self>
    where
        E: Executor<'e>,
    {
        crate::model::writes::first_or_insert(
            self,
            executor,
            columns.into_iter().map(|c| c.name()).collect(),
        )
        .await
        .map_err(crate::constraint::as_field_error::<Self>)
    }

    /// Add `by` to `column` of this record's row in the database
    /// (`SET column = column + by`, without reading it first) and return
    /// the stored row; `updated_at` and the version move as for any update.
    async fn increment<'e, E>(
        &self,
        executor: E,
        column: Column<Self>,
        by: impl Into<Value>,
    ) -> Result<Self>
    where
        E: Executor<'e>,
    {
        Self::filter(self.key_filter())
            .update()
            .increment(column, by)
            .returning_one_or_fail(executor)
            .await
    }

    /// Subtract `by` from `column`, like [`increment`](Self::increment).
    async fn decrement<'e, E>(
        &self,
        executor: E,
        column: Column<Self>,
        by: impl Into<Value>,
    ) -> Result<Self>
    where
        E: Executor<'e>,
    {
        Self::filter(self.key_filter())
            .update()
            .decrement(column, by)
            .returning_one_or_fail(executor)
            .await
    }

    /// Set this record's `updated_at` to `now()` and return the stored row.
    /// A model without an `updated_at` column is an
    /// [`Error::InvalidQuery`].
    async fn touch<'e, E>(&self, executor: E) -> Result<Self>
    where
        E: Executor<'e>,
    {
        let Some(updated_at) = Self::UPDATED_AT_COLUMN else {
            return Err(Error::InvalidQuery(format!(
                "`{}` has no `updated_at` column to touch",
                Self::TABLE
            )));
        };
        Self::filter(self.key_filter())
            .update()
            .set_raw(Column::new(updated_at), "now()", [] as [Value; 0])
            .returning_one_or_fail(executor)
            .await
    }

    /// `UPDATE` every non-generated column of this record by primary key and
    /// return the stored row.
    async fn update<'e, E>(&self, executor: E) -> Result<Self>
    where
        E: Executor<'e>,
    {
        let pre = crate::model::writes::pre_save(self);
        crate::model::writes::update_only(self, executor, None, pre)
            .await
            .map_err(crate::constraint::as_field_error::<Self>)
    }

    /// Like `update`, but only write `columns` (managed columns such as
    /// `updated_at` and `version` are still maintained):
    /// `user.update_only(&db, [User::NAME])`. An empty list is an
    /// [`Error::InvalidQuery`], never a silent full `update`.
    async fn update_only<'e, E>(
        &self,
        executor: E,
        columns: impl IntoIterator<Item = Column<Self>>,
    ) -> Result<Self>
    where
        E: Executor<'e>,
    {
        let columns: Vec<Column<Self>> = columns.into_iter().collect();
        if columns.is_empty() {
            return Err(Error::InvalidQuery(format!(
                "`update_only` on `{}` names no column; `update` writes every column",
                Self::TABLE
            )));
        }
        let pre = crate::model::writes::pre_save(self);
        crate::model::writes::update_only(self, executor, Some(columns), pre)
            .await
            .map_err(crate::constraint::as_field_error::<Self>)
    }

    /// Delete this record by primary key.
    async fn delete<'e, E>(&self, executor: E) -> Result<()>
    where
        E: Executor<'e>,
    {
        let pre = self.before_delete();
        crate::model::writes::delete_by_key(self, executor, false, pre).await
    }

    /// Permanently `DELETE` this record, even for models with soft deletes.
    async fn force_delete<'e, E>(&self, executor: E) -> Result<()>
    where
        E: Executor<'e>,
    {
        let pre = self.before_delete();
        crate::model::writes::delete_by_key(self, executor, true, pre).await
    }

    /// Restore this soft-deleted record (`deleted_at = NULL`) and return the
    /// stored row.
    async fn restore<'e, E>(&self, executor: E) -> Result<Self>
    where
        E: Executor<'e>,
    {
        let key = crate::key::key_string(&self.key_values());
        let update = Self::DELETED_AT_COLUMN.map(|deleted_at| {
            Self::filter(self.key_filter())
                .only_trashed()
                .unscoped()
                .update()
                .set_raw(
                    crate::expr::Column::new(deleted_at),
                    "NULL",
                    [] as [Value; 0],
                )
        });
        let Some(update) = update else {
            return Err(Error::InvalidQuery(format!(
                "`{}` has no soft-delete column",
                Self::TABLE
            )));
        };
        update
            .returning_one(executor)
            .await?
            .ok_or_else(|| Error::not_found::<Self>(Some(key)))
    }
}
