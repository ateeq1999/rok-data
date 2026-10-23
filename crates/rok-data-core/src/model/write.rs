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
        crate::model::writes::insert(self, executor).await
    }

    /// `INSERT` many records in a single statement and return the stored rows.
    async fn insert_all<'e, E>(executor: E, records: &[Self]) -> Result<Vec<Self>>
    where
        E: Executor<'e>,
    {
        Self::insert_many(records).exec(executor).await
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
        crate::model::writes::upsert(self, executor).await
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
    }

    /// `UPDATE` every non-generated column of this record by primary key and
    /// return the stored row.
    async fn update<'e, E>(&self, executor: E) -> Result<Self>
    where
        E: Executor<'e>,
    {
        let pre = crate::model::writes::pre_save(self);
        crate::model::writes::update_only(self, executor, None, pre).await
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
        crate::model::writes::update_only(self, executor, Some(columns), pre).await
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
