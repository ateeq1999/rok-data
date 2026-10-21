//! The row-level write mechanics behind the [`Model`](crate::Model) verbs.
//!
//! The trait's methods are the API; the statements they run live here, so
//! the trait file stays a surface rather than a body.

use crate::exec;
use crate::key::key_string;
use crate::model::checked_write::checked_write;
use crate::model::insert::{insert_sql, is_timestamp, pre_insert, writable};
use crate::query::{Conflict, ConflictAction, ConflictTarget};
use crate::sql::Sql;
use crate::{Column, Error, Executor, Model, Result, Value};

/// `INSERT` the record (skipping generated columns) and return the stored
/// row.
pub(crate) async fn insert<'e, M, E>(record: &M, executor: E) -> Result<M>
where
    M: Model,
    E: Executor<'e>,
{
    // Rendered when awaited, so tenant scopes apply.
    pre_insert(record)?;
    let sql = insert_sql::<M>(std::slice::from_ref(record), false, None);
    let stored = exec::fetch_optional::<M, _>(executor, &sql?, &[M::TABLE], false)
        .await?
        .ok_or_else(|| Error::not_found::<M>(None))?;
    stored.after_insert()?;
    Ok(stored)
}

/// `INSERT` the record, or update every column if its primary key already
/// exists (`ON CONFLICT (pk) DO UPDATE`).
pub(crate) async fn upsert<'e, M, E>(record: &M, executor: E) -> Result<M>
where
    M: Model,
    E: Executor<'e>,
{
    let conflict = Conflict {
        target: ConflictTarget::Columns(M::PRIMARY_KEYS.to_vec()),
        action: ConflictAction::UpdateAll,
    };
    pre_insert(record)?;
    let sql = insert_sql::<M>(std::slice::from_ref(record), true, Some(&conflict));
    let stored = exec::fetch_optional::<M, _>(executor, &sql?, &[M::TABLE], false)
        .await?
        .ok_or_else(|| Error::not_found::<M>(None))?;
    stored.after_insert()?;
    Ok(stored)
}

/// `INSERT` the record, or update it if it conflicts on `columns` (a unique
/// index).
pub(crate) async fn upsert_on<'e, M, E>(
    record: &M,
    executor: E,
    columns: Vec<&'static str>,
) -> Result<M>
where
    M: Model,
    E: Executor<'e>,
{
    let conflict = Conflict {
        target: ConflictTarget::Columns(columns),
        action: ConflictAction::UpdateAll,
    };
    pre_insert(record)?;
    let sql = insert_sql::<M>(std::slice::from_ref(record), false, Some(&conflict));
    let stored = exec::fetch_optional::<M, _>(executor, &sql?, &[M::TABLE], false)
        .await?
        .ok_or_else(|| Error::not_found::<M>(None))?;
    stored.after_insert()?;
    Ok(stored)
}

/// `UPDATE` the record's columns by primary key and return the stored row.
///
/// `pre` is the validation and the `before_save` hook, already run by the
/// caller: the hook fires when the verb is called, not when the future is
/// awaited.
pub(crate) async fn update_only<'e, M, E>(
    record: &M,
    executor: E,
    columns: Option<Vec<Column<M>>>,
    pre: Result<()>,
) -> Result<M>
where
    M: Model,
    E: Executor<'e>,
{
    let only: Option<Vec<&'static str>> = columns.map(|c| c.iter().map(|c| c.name()).collect());
    let key = key_string(&record.key_values());
    let managed = |c: &str| {
        M::PRIMARY_KEYS.contains(&c)
            || is_timestamp::<M>(c)
            || M::VERSION_COLUMN == Some(c)
            || M::TENANT_COLUMN == Some(c)
    };
    let sets: Vec<_> = writable::<M>(record.values())
        .filter(|(c, _)| !managed(c))
        .filter(|(c, _)| only.as_ref().is_none_or(|only| only.contains(c)))
        .collect();
    let mut update = M::update_all()
        .with_trashed()
        .unscoped()
        .filter(record.key_filter());
    let version = M::VERSION_COLUMN.and_then(|c| record.value_of(c).map(|v| (c, v)));
    if let Some((column, current)) = version.clone() {
        update = update.filter(Column::new(column).eq(current));
    }
    for (column, value) in sets {
        update = update.set(Column::new(column), value);
    }
    let changes =
        update.has_sets() || M::UPDATED_AT_COLUMN.is_some() || M::VERSION_COLUMN.is_some();
    pre?;
    let row = if !changes {
        update
            .into_select()
            .first(executor)
            .await?
            .ok_or_else(|| Error::not_found::<M>(Some(key)))?
    } else if version.is_some() {
        let mut sql = Sql::new();
        update.write_into(&mut sql);
        let exists = M::filter(record.key_filter()).with_trashed().unscoped();
        checked_write::<M, _>(executor, sql, key, exists).await?
    } else {
        update
            .returning_one(executor)
            .await?
            .ok_or_else(|| Error::not_found::<M>(Some(key)))?
    };
    row.after_save()?;
    Ok(row)
}

/// `DELETE` the record by primary key, or mark it deleted when the model
/// has soft deletes.
///
/// `pre` is the `before_delete` hook, already run by the caller.
pub(crate) async fn delete_by_key<'e, M, E>(
    record: &M,
    executor: E,
    force: bool,
    pre: Result<()>,
) -> Result<()>
where
    M: Model,
    E: Executor<'e>,
{
    let key = key_string(&record.key_values());
    let soft = M::DELETED_AT_COLUMN.filter(|_| !force);
    let version = M::VERSION_COLUMN.and_then(|c| record.value_of(c).map(|v| (c, v)));
    let mut select = M::filter(record.key_filter()).unscoped();
    if force {
        select = select.with_trashed();
    }
    // "Does the row exist?" uses the same soft-delete scope as the write.
    let exists = select.clone();
    if let Some((column, current)) = version.clone() {
        select = select.filter(Column::new(column).eq(current));
    }
    pre?;
    // Rendered when awaited, so tenant scopes apply.
    let mut sql = Sql::new();
    match soft {
        Some(deleted_at) => select
            .update()
            .set_raw(Column::new(deleted_at), "now()", [] as [Value; 0])
            .write_into(&mut sql),
        None => {
            let delete = if force {
                select.force_delete_sql()
            } else {
                select.delete_sql()
            };
            sql = delete;
        }
    }
    if version.is_some() {
        checked_write::<M, _>(executor, sql, key, exists).await?;
    } else if exec::execute(executor, &sql, &[M::TABLE]).await? == 0 {
        return Err(Error::not_found::<M>(Some(key)));
    }
    record.after_delete()
}
