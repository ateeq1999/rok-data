//! The conventions a generated row carries: the managed timestamps and the
//! soft delete the column names say, and the opt-outs that turn them off
//! (workspace ADR 0015).
//!
//! The row's field names are the signal, exactly as they are for a
//! hand-written model: a table with `created_at` and `updated_at` columns
//! gets managed timestamps, a table with a `deleted_at` column soft-deletes.
//! The generator writes the opt-out where the database owns the column, and
//! nothing where the inference already says the right thing.

use crate::schema::{Column, Table};

/// The column name the soft delete is inferred from.
const DELETED_AT: &str = "deleted_at";
/// The column names the managed timestamps are inferred from.
const CREATED_AT: &str = "created_at";
const UPDATED_AT: &str = "updated_at";

/// The conventions a generated row carries.
pub(crate) struct Conventions<'a> {
    /// The soft-delete column the row names with the field marker, when the
    /// inference cannot name it: the column's field is not called
    /// `deleted_at`, so the row must say which column it is.
    pub(crate) soft_delete: Option<&'a Column>,
    /// The row opts out of the inferred timestamps.
    pub(crate) no_timestamps: bool,
    /// The row opts out of the inferred soft delete.
    pub(crate) no_soft_delete: bool,
}

/// The conventions the row of `table` carries.
pub(crate) fn decide(table: &Table) -> Conventions<'_> {
    let soft_delete = named_soft_delete(table);
    let inferred = inferred_soft_delete(table);
    Conventions {
        soft_delete,
        no_timestamps: !manages_timestamps(table),
        // The soft-delete convention is explicit and wins; the inference
        // names the `deleted_at` column, and the opt-out turns it off where
        // the database owns that column.
        no_soft_delete: soft_delete.is_none()
            && inferred.is_some_and(|column| the_database_owns(column)),
    }
}

/// The soft-delete column the conventions name, when the row's field name
/// does not say it already: the field marker names the column, so the
/// inference finds it.
fn named_soft_delete(table: &Table) -> Option<&Column> {
    let name = table.soft_delete_column.as_deref()?;
    let column = table
        .columns
        .iter()
        .find(|column| column.sql_name == name)?;
    (column.rust_name != DELETED_AT).then_some(column)
}

/// The column the inference names: the one called `deleted_at`.
fn inferred_soft_delete(table: &Table) -> Option<&Column> {
    table
        .columns
        .iter()
        .find(|column| column.sql_name == DELETED_AT)
}

/// Whether the database owns `column` and not the data layer: it fills the
/// column with a value of its own, or it cannot carry a soft delete at all.
fn the_database_owns(column: &Column) -> bool {
    column.is_generated || fills_with_a_value(column) || !is_soft_delete_shape(column)
}

/// Whether the table's timestamp columns are the data layer's to manage:
/// each of them is a managed column (the `managed_columns` convention) or
/// defaults to the current time.
fn manages_timestamps(table: &Table) -> bool {
    let column = |name: &str| table.columns.iter().find(|column| column.sql_name == name);
    match (column(CREATED_AT), column(UPDATED_AT)) {
        (Some(created_at), Some(updated_at)) => {
            data_layer_manages_timestamp(created_at) && data_layer_manages_timestamp(updated_at)
        }
        // One timestamp column alone is not the convention: the model would
        // manage a half-pair.
        _ => true,
    }
}

/// Whether the data layer manages `column` as a timestamp: the
/// `managed_columns` convention says so, or the column defaults to the
/// current time (so writing `now()` keeps the database's value). A column
/// the database always owns is not the data layer's.
fn data_layer_manages_timestamp(column: &Column) -> bool {
    !column.is_generated && (column.is_managed || defaults_to_now(column))
}

/// Whether the column's default is the current time.
fn defaults_to_now(column: &Column) -> bool {
    column.default.as_deref().is_some_and(|default| {
        let default = default.trim().to_ascii_lowercase();
        matches!(
            default.as_str(),
            "now()" | "current_timestamp" | "transaction_timestamp()" | "localtimestamp"
        )
    })
}

/// Whether the database fills `column` with a value of its own: its default
/// is not `NULL`.
fn fills_with_a_value(column: &Column) -> bool {
    column
        .default
        .as_deref()
        .is_some_and(|default| !default.trim().eq_ignore_ascii_case("null"))
}

/// Whether `column` is a column a soft delete can use: a nullable timestamp.
fn is_soft_delete_shape(column: &Column) -> bool {
    column.nullable && column.base_type().is_timestamp()
}
