//! The `Model` trait: the columns, the keys, and the single-row operations.

use std::future::Future;

use sqlx::FromRow;
use sqlx::postgres::PgRow;

use super::Hooks;
use super::writes;
use crate::key::{IntoKey, key_expr, key_string};
use crate::query::{Insert, InsertMany, Select, Update};
use crate::validate::ValidationErrors;
use crate::{Column, Error, Executor, Expr, Order, Result, Value};

/// A database-backed record.
///
/// Implement it with `#[derive(Model)]` rather than by hand: the derive
/// generates the metadata below, a [`FromRow`] implementation and one typed
/// [`Column`] constant per field.
///
/// All query and persistence methods take any [`Executor`]: a [`Db`](crate::Db),
/// a `&mut Tx` transaction, a raw `PgPool` or a `&mut PgConnection`.
///
/// ```ignore
/// let user = User::find_or_fail(&db, 1).await?;
/// let admins = User::filter(User::ROLE.eq("admin"))
///     .order_by(User::NAME.asc())
///     .all(&db)
///     .await?;
/// ```
pub trait Model:
    for<'r> FromRow<'r, PgRow> + Hooks + Send + Sync + Unpin + Sized + 'static
{
    /// Table name, optionally schema-qualified (`"auth.users"`).
    const TABLE: &'static str;
    /// Primary key column (the first one, for composite keys).
    const PRIMARY_KEY: &'static str;
    /// Every primary key column, in order (more than one for composite keys).
    const PRIMARY_KEYS: &'static [&'static str] = &[Self::PRIMARY_KEY];
    /// Every column read from and written to the database, in field order.
    const COLUMNS: &'static [&'static str];
    /// Columns filled in by the database (serial ids, defaults, triggers).
    /// They are read but never written by [`insert`](Model::insert) or [`update`](Model::update).
    const GENERATED: &'static [&'static str] = &[];
    /// Column set to `now()` on insert and never changed afterwards.
    const CREATED_AT_COLUMN: Option<&'static str> = None;
    /// Column set to `now()` on insert and on every update.
    const UPDATED_AT_COLUMN: Option<&'static str> = None;
    /// Soft-delete column: `delete` sets it to `now()` and queries skip rows
    /// where it isn't `NULL`.
    const DELETED_AT_COLUMN: Option<&'static str> = None;
    /// Tenant column: see [`tenant`](crate::tenant).
    const TENANT_COLUMN: Option<&'static str> = None;
    /// Optimistic-locking column: incremented on every update, and `update`/
    /// `delete` fail with [`Error::Conflict`] if it changed since loading.
    const VERSION_COLUMN: Option<&'static str> = None;

    /// The value of this record's primary key (its first column, for
    /// composite keys; see [`key_values`](Model::key_values)).
    fn primary_key(&self) -> Value;

    /// The values of every primary key column, in order.
    fn key_values(&self) -> Vec<Value> {
        Self::PRIMARY_KEYS
            .iter()
            .map(|c| self.value_of(c).unwrap_or(Value::I64(None)))
            .collect()
    }

    /// A condition matching this record by its (possibly composite) key.
    fn key_filter(&self) -> Expr<Self> {
        crate::key::key_expr::<Self>(self.key_values()).unwrap_or_else(|_| Expr::none())
    }

    /// Every column with its current value, in [`COLUMNS`](Model::COLUMNS) order.
    fn values(&self) -> Vec<(&'static str, Value)>;

    /// Check the record's `#[rok(validate(…))]` rules. Called automatically
    /// before every insert and update.
    fn validate(&self) -> std::result::Result<(), ValidationErrors> {
        Ok(())
    }

    /// Decode the model from [`COLUMNS`](Model::COLUMNS) starting at
    /// position `offset` of `row` (used to fetch joined models as tuples).
    /// The derive generates it; without `FromRow` from the derive it fails.
    #[doc(hidden)]
    fn from_row_at(row: &PgRow, offset: usize) -> std::result::Result<Self, sqlx::Error> {
        let _ = (row, offset);
        Err(sqlx::Error::ColumnNotFound(format!(
            "`{}` can't be decoded by position (it uses `#[rok(no_from_row)]`)",
            Self::TABLE
        )))
    }

    /// A condition applied to every query of this model (like soft deletes),
    /// set with `#[rok(default_scope = path::to::fn)]`. Remove it per query
    /// with [`Select::unscoped`].
    fn default_scope() -> Option<Expr<Self>> {
        None
    }

    /// The current value of a single column, or `None` if `column` isn't one
    /// of [`COLUMNS`](Model::COLUMNS). The derive generates an efficient
    /// implementation.
    fn value_of(&self, column: &str) -> Option<Value> {
        self.values()
            .into_iter()
            .find(|(c, _)| *c == column)
            .map(|(_, v)| v)
    }

    // ----- query entry points ---------------------------------------------

    /// Start a `SELECT` over every row of the table.
    fn query() -> Select<Self> {
        Select::new()
    }

    /// Start a `SELECT` filtered by `expr`.
    fn filter(expr: Expr<Self>) -> Select<Self> {
        Select::new().filter(expr)
    }

    /// Start a `SELECT` filtered by `expr` — the Eloquent spelling of
    /// [`filter`](Model::filter).
    ///
    /// `where` is a Rust keyword, so only the raw identifier `r#where`
    /// compiles; `filter` stays the canonical verb.
    ///
    /// ```ignore
    /// let admins = User::r#where(User::ROLE.eq("admin")).all(&db).await?;
    /// ```
    fn r#where(expr: Expr<Self>) -> Select<Self> {
        Self::filter(expr)
    }

    /// Start a `SELECT` ordered by `order`.
    fn order_by(order: impl Into<Order<Self>>) -> Select<Self> {
        Select::new().order_by(order)
    }

    /// Start an `INSERT` built column by column — the low-level write path,
    /// for raw expressions and conflict handling.
    ///
    /// ```ignore
    /// User::insert_builder()
    ///     .set(User::EMAIL, "a@b.c")
    ///     .exec(&db)
    ///     .await?;
    /// ```
    fn insert_builder() -> Insert<Self> {
        Insert::new()
    }

    /// Start an `INSERT` of whole records, e.g. to configure `ON CONFLICT`:
    ///
    /// ```ignore
    /// User::insert_many(&users)
    ///     .on_conflict([User::EMAIL])
    ///     .do_update([User::NAME])
    ///     .exec(&db)
    ///     .await?;
    /// ```
    fn insert_many(records: &[Self]) -> InsertMany<'_, Self> {
        InsertMany::new(records)
    }

    /// Start a bulk `UPDATE`; narrow it with [`Update::filter`].
    fn update_all() -> Update<Self> {
        Update::new()
    }

    /// The primary key as a typed column.
    fn primary_key_column() -> Column<Self> {
        Column::new(Self::PRIMARY_KEY)
    }

    // ----- reads ----------------------------------------------------------

    /// Fetch a record by primary key, or `None`.
    fn find<'e, E>(
        executor: E,
        id: impl IntoKey,
    ) -> impl Future<Output = Result<Option<Self>>> + Send
    where
        E: Executor<'e>,
    {
        let select = key_expr::<Self>(id.into_key()).map(Self::filter);
        async move { select?.first(executor).await }
    }

    /// Fetch a record by primary key, failing with [`Error::NotFound`].
    fn find_or_fail<'e, E>(
        executor: E,
        id: impl IntoKey,
    ) -> impl Future<Output = Result<Self>> + Send
    where
        E: Executor<'e>,
    {
        let values = id.into_key();
        let key = key_string(&values);
        let select = key_expr::<Self>(values).map(Self::filter);
        async move {
            select?
                .first(executor)
                .await?
                .ok_or_else(|| Error::not_found::<Self>(Some(key)))
        }
    }

    /// Fetch every record whose primary key is in `ids`.
    fn find_many<'e, E, K>(
        executor: E,
        ids: impl IntoIterator<Item = K>,
    ) -> impl Future<Output = Result<Vec<Self>>> + Send
    where
        E: Executor<'e>,
        K: IntoKey,
    {
        let keys: Vec<Vec<Value>> = ids.into_iter().map(IntoKey::into_key).collect();
        let select = if Self::PRIMARY_KEYS.len() == 1 {
            Ok(Self::filter(
                Self::primary_key_column().is_in(keys.into_iter().flatten()),
            ))
        } else {
            keys.into_iter()
                .map(key_expr::<Self>)
                .collect::<Result<Vec<_>>>()
                .map(|exprs| Self::filter(Expr::any_of(exprs)))
        };
        async move { select?.all(executor).await }
    }

    /// Fetch every record in the table.
    fn all<'e, E>(executor: E) -> impl Future<Output = Result<Vec<Self>>> + Send
    where
        E: Executor<'e>,
    {
        Self::query().all(executor)
    }

    /// Count every record in the table.
    fn count<'e, E>(executor: E) -> impl Future<Output = Result<i64>> + Send
    where
        E: Executor<'e>,
    {
        Self::query().count(executor)
    }

    /// `true` if the table has at least one record.
    ///
    /// The model's own scope applies (soft deletes, the default scope, the
    /// tenant), like every other read.
    ///
    /// ```ignore
    /// if User::exists(&db).await? {
    ///     // …
    /// }
    /// ```
    fn exists<'e, E>(executor: E) -> impl Future<Output = Result<bool>> + Send
    where
        E: Executor<'e>,
    {
        Self::query().exists(executor)
    }

    /// Re-read this record from the database.
    fn reload<'e, E>(&self, executor: E) -> impl Future<Output = Result<Self>> + Send
    where
        E: Executor<'e>,
    {
        // Reloading a soft-deleted record still works.
        let missing = key_string(&self.key_values());
        let select = Self::filter(self.key_filter()).with_trashed().unscoped();
        async move {
            select
                .first(executor)
                .await?
                .ok_or_else(|| Error::not_found::<Self>(Some(missing)))
        }
    }

    /// Track changes to this record so [`Tracked::save`](crate::Tracked::save)
    /// writes only modified columns.
    fn track(self) -> crate::Tracked<Self> {
        crate::Tracked::new(self)
    }

    /// Install (or replace) an `AFTER INSERT OR UPDATE OR DELETE` trigger
    /// that announces every row change of this table, for
    /// [`changes`](Model::changes). Run it once, e.g. from a migration or at
    /// startup (PostgreSQL 11+).
    fn install_change_notifications(db: &crate::Db) -> impl Future<Output = Result<()>> + Send {
        let sql = crate::notify::install_sql::<Self>();
        async move {
            db.execute(&sql?).await?;
            Ok(())
        }
    }

    /// Remove the trigger installed by
    /// [`install_change_notifications`](Model::install_change_notifications).
    fn uninstall_change_notifications(db: &crate::Db) -> impl Future<Output = Result<()>> + Send {
        let sql = crate::notify::uninstall_sql::<Self>();
        async move {
            db.execute(&sql).await?;
            Ok(())
        }
    }

    /// Subscribe to row changes of this table (see
    /// [`install_change_notifications`](Model::install_change_notifications)).
    fn changes(
        db: &crate::Db,
    ) -> impl Future<Output = Result<crate::notify::ChangeStream<Self>>> + Send {
        async move {
            let listener = db.listen(&[&crate::notify::channel::<Self>()]).await?;
            Ok(crate::notify::ChangeStream::new(listener))
        }
    }

    /// `true` if this record has been soft-deleted.
    fn is_trashed(&self) -> bool {
        Self::DELETED_AT_COLUMN
            .and_then(|c| self.value_of(c))
            .is_some_and(|v| !v.is_null())
    }

    // ----- writes ---------------------------------------------------------

    /// `INSERT` this record (skipping generated columns) and return the stored row.
    fn insert<'e, E>(&self, executor: E) -> impl Future<Output = Result<Self>> + Send
    where
        E: Executor<'e>,
    {
        writes::insert(self, executor)
    }

    /// `INSERT` many records in a single statement and return the stored rows.
    fn insert_all<'e, E>(
        executor: E,
        records: &[Self],
    ) -> impl Future<Output = Result<Vec<Self>>> + Send
    where
        E: Executor<'e>,
    {
        Self::insert_many(records).exec(executor)
    }

    /// Bulk-load `records` with PostgreSQL's binary `COPY`, typically 5–20×
    /// faster than `INSERT` for large batches. Returns the number of rows
    /// written.
    ///
    /// ```ignore
    /// User::copy_in(&db, &users).await?;          // borrows a pooled connection
    /// User::copy_in(&mut *tx, &users).await?;     // inside a transaction
    /// ```
    ///
    /// Records are validated and passed to `before_insert` first;
    /// `after_insert` is not called and nothing is returned, because `COPY`
    /// doesn't return rows. Generated columns are skipped; every other value
    /// (timestamps and `version` included) is written as-is. Binary `COPY`
    /// needs each field's Rust type to match its column type exactly (e.g.
    /// `i64` for `BIGINT`). The table's cached query results are invalidated
    /// (except when copying through a plain `PgConnection`).
    fn copy_in<'a>(
        target: impl Into<crate::copy::CopyTarget<'a>>,
        records: &[Self],
    ) -> impl Future<Output = Result<u64>> + Send {
        let target = target.into();
        async move { crate::copy::copy_in(target, records).await }
    }

    /// `INSERT` this record, or update every column if its primary key
    /// already exists (`ON CONFLICT (pk) DO UPDATE`). The primary key is
    /// always written, even when it is a generated column.
    fn upsert<'e, E>(&self, executor: E) -> impl Future<Output = Result<Self>> + Send
    where
        E: Executor<'e>,
    {
        writes::upsert(self, executor)
    }

    /// `INSERT` this record, or update it if it conflicts on `columns` (a
    /// unique index), e.g. upsert by email:
    ///
    /// ```ignore
    /// let user = new_user.upsert_on(&db, [User::EMAIL]).await?;
    /// ```
    ///
    /// Every inserted column except `columns`, the primary key and
    /// `created_at` is overwritten. For other policies use
    /// [`insert_many`](Model::insert_many) or
    /// [`insert_builder`](Model::insert_builder).
    fn upsert_on<'e, E>(
        &self,
        executor: E,
        columns: impl IntoIterator<Item = Column<Self>>,
    ) -> impl Future<Output = Result<Self>> + Send
    where
        E: Executor<'e>,
    {
        writes::upsert_on(
            self,
            executor,
            columns.into_iter().map(|c| c.name()).collect(),
        )
    }

    /// `UPDATE` every non-generated column of this record by primary key and
    /// return the stored row. Fails with [`Error::NotFound`] if the row is
    /// gone, and with [`Error::Conflict`] if its `#[rok(version)]` changed
    /// since it was loaded.
    ///
    /// ```ignore
    /// user.name = "Ann".into();
    /// let user = user.update(&db).await?;
    /// ```
    fn update<'e, E>(&self, executor: E) -> impl Future<Output = Result<Self>> + Send
    where
        E: Executor<'e>,
    {
        self.update_only(executor, None)
    }

    /// Like [`update`](Model::update), but only write `columns` (managed
    /// columns such as `updated_at` and `version` are still maintained).
    /// Passing `None` writes every column.
    ///
    /// ```ignore
    /// user.update_only(&db, Some(vec![User::EMAIL])).await?;
    /// ```
    ///
    /// [`Tracked`](crate::Tracked) uses this to write only changed fields.
    fn update_only<'e, E>(
        &self,
        executor: E,
        columns: Option<Vec<Column<Self>>>,
    ) -> impl Future<Output = Result<Self>> + Send
    where
        E: Executor<'e>,
    {
        // The validation and the `before_save` hook run when the call is
        // made, not when the future is awaited.
        let pre = self
            .validate()
            .map_err(Error::Validation)
            .and_then(|()| self.before_save());
        writes::update_only(self, executor, columns, pre)
    }

    /// Delete this record by primary key. Models with soft deletes are
    /// marked deleted (`deleted_at = now()`); use
    /// [`force_delete`](Model::force_delete) to remove the row.
    ///
    /// Fails with [`Error::NotFound`] if no row was deleted, and with
    /// [`Error::Conflict`] if its `#[rok(version)]` changed since it was
    /// loaded.
    fn delete<'e, E>(&self, executor: E) -> impl Future<Output = Result<()>> + Send
    where
        E: Executor<'e>,
    {
        self.delete_by_key(executor, false)
    }

    /// Permanently `DELETE` this record, even for models with soft deletes.
    fn force_delete<'e, E>(&self, executor: E) -> impl Future<Output = Result<()>> + Send
    where
        E: Executor<'e>,
    {
        self.delete_by_key(executor, true)
    }

    #[doc(hidden)]
    fn delete_by_key<'e, E>(
        &self,
        executor: E,
        force: bool,
    ) -> impl Future<Output = Result<()>> + Send
    where
        E: Executor<'e>,
    {
        // The `before_delete` hook runs when the call is made, not when the
        // future is awaited.
        let pre = self.before_delete();
        writes::delete_by_key(self, executor, force, pre)
    }

    /// Restore this soft-deleted record (`deleted_at = NULL`) and return the
    /// stored row.
    fn restore<'e, E>(&self, executor: E) -> impl Future<Output = Result<Self>> + Send
    where
        E: Executor<'e>,
    {
        let key = key_string(&self.key_values());
        let update = Self::DELETED_AT_COLUMN.map(|deleted_at| {
            Self::filter(self.key_filter())
                .only_trashed()
                .unscoped()
                .update()
                .set_raw(Column::new(deleted_at), "NULL", [] as [Value; 0])
        });
        async move {
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
}
