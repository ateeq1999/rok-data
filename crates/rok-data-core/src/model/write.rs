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
        self.update_only(executor, None).await
    }

    /// Like `update`, but only write `columns` (managed columns such as
    /// `updated_at` and `version` are still maintained).
    async fn update_only<'e, E>(
        &self,
        executor: E,
        columns: Option<Vec<Column<Self>>>,
    ) -> Result<Self>
    where
        E: Executor<'e>,
    {
        let pre = self
            .validate()
            .map_err(Error::Validation)
            .and_then(|()| self.before_save());
        crate::model::writes::update_only(self, executor, columns, pre).await
    }

    /// Delete this record by primary key.
    async fn delete<'e, E>(&self, executor: E) -> Result<()>
    where
        E: Executor<'e>,
    {
        self.delete_by_key(executor, false).await
    }

    /// Permanently `DELETE` this record, even for models with soft deletes.
    async fn force_delete<'e, E>(&self, executor: E) -> Result<()>
    where
        E: Executor<'e>,
    {
        self.delete_by_key(executor, true).await
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

    /// Internal: delete by key with force option.
    #[doc(hidden)]
    async fn delete_by_key<'e, E>(&self, executor: E, force: bool) -> Result<()>
    where
        E: Executor<'e>,
    {
        let pre = self.before_delete();
        crate::model::writes::delete_by_key(self, executor, force, pre).await
    }
}
