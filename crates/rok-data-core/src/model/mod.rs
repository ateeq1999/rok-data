//! The model contract: what a row type promises the runtime.

use crate::Result;

mod checked_write;
mod insert;
mod row;
mod writes;

pub use insert::__insert_sql;
pub(crate) use insert::{insert_sql, pre_insert, push_columns, push_returning, tenant_override};
pub use row::Model;

/// Lifecycle hooks, called around the record-level writes of [`Model`]
/// (`insert`, `insert_all`/`insert_many`, `upsert`, `update`, `delete`,
/// `force_delete`). Bulk query operations (`Select::update`,
/// `Select::delete`, …) don't call hooks.
///
/// `#[derive(Model)]` implements this trait with no-op hooks; add
/// `#[rok(hooks)]` to implement it yourself:
///
/// ```ignore
/// #[derive(Model)]
/// #[rok(hooks)]
/// struct User { id: i64, email: String }
///
/// impl Hooks for User {
///     fn before_insert(&self) -> rok_data::Result<()> {
///         if self.email.ends_with("@blocked.example") {
///             return Err(rok_data::Error::hook("this domain is blocked"));
///         }
///         Ok(())
///     }
/// }
/// ```
///
/// A `before_*` error aborts the operation before anything is written.
/// An `after_*` error is returned to the caller *after* the write happened;
/// run the operation in a transaction if it should be rolled back.
pub trait Hooks {
    /// Before inserting (also upserts), after validation.
    fn before_insert(&self) -> Result<()> {
        Ok(())
    }
    /// After inserting, with the stored row.
    fn after_insert(&self) -> Result<()> {
        Ok(())
    }
    /// Before the record is written by the `update` verb, after validation.
    fn before_save(&self) -> Result<()> {
        Ok(())
    }
    /// After the record is written by the `update` verb, with the stored row.
    fn after_save(&self) -> Result<()> {
        Ok(())
    }
    /// Before `delete` / `force_delete`.
    fn before_delete(&self) -> Result<()> {
        Ok(())
    }
    /// After `delete` / `force_delete`.
    fn after_delete(&self) -> Result<()> {
        Ok(())
    }
}
