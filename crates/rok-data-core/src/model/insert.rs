//! Building the `INSERT` statements and the column lists they return.

use crate::query::{Conflict, ConflictAction, ConflictTarget};
use crate::sql::Sql;
use crate::{Error, Result, Value};

use super::row::Model;

/// Validation and `before_insert`, run before any insert of `record`.
pub(crate) fn pre_insert<M: Model>(record: &M) -> Result<()> {
    record.validate().map_err(Error::Validation)?;
    record.before_insert()
}

pub(crate) fn writable<M: Model>(
    values: Vec<(&'static str, Value)>,
) -> impl Iterator<Item = (&'static str, Value)> {
    values
        .into_iter()
        .filter(|(c, _)| !M::GENERATED.contains(c))
}

pub(crate) fn is_timestamp<M: Model>(column: &str) -> bool {
    M::CREATED_AT_COLUMN == Some(column) || M::UPDATED_AT_COLUMN == Some(column)
}

/// Build `INSERT … VALUES (…), (…) [ON CONFLICT …] RETURNING …`.
pub(crate) fn insert_sql<M: Model>(
    records: &[M],
    include_pk: bool,
    conflict: Option<&Conflict>,
) -> Result<Sql> {
    let include =
        |c: &str| !M::GENERATED.contains(&c) || (include_pk && M::PRIMARY_KEYS.contains(&c));
    let columns: Vec<&'static str> = M::COLUMNS.iter().copied().filter(|c| include(c)).collect();

    let mut sql = Sql::new();
    sql.push("INSERT INTO ").push_ident(M::TABLE);
    if columns.is_empty() {
        // Every column is generated: rely on defaults (single row only).
        sql.push(" DEFAULT VALUES");
    } else {
        sql.push(" (")
            .push_list(&columns, ", ", |sql, c| {
                sql.push_ident(c);
            })
            .push(") VALUES ");
        let tenant = M::TENANT_COLUMN.zip(crate::tenant::current());
        sql.push_list(records, ", ", |sql, record| {
            let values = record.values().into_iter().filter(|(c, _)| include(c));
            sql.push("(")
                .push_list(values, ", ", |sql, (c, v)| {
                    if is_timestamp::<M>(c) {
                        sql.push("now()");
                    } else if let Some((_, current)) = tenant.as_ref().filter(|(t, _)| *t == c) {
                        sql.bind(current.clone());
                    } else {
                        sql.bind(v);
                    }
                })
                .push(")");
        });
    }
    if let Some(conflict) = conflict {
        conflict.write::<M>(&mut sql, &columns)?;
    }
    push_returning::<M>(&mut sql);
    Ok(sql)
}

/// The tenant value to write for `column` of `M`, if it is the tenant column
/// and a tenant scope is active.
pub(crate) fn tenant_override<M: Model>(column: &str) -> Option<Value> {
    M::TENANT_COLUMN
        .filter(|c| *c == column)
        .and_then(|_| crate::tenant::current())
}

pub(crate) fn push_columns<M: Model>(sql: &mut Sql) {
    sql.push_list(M::COLUMNS, ", ", |sql, c| {
        sql.push_column(M::TABLE, c);
    });
}

pub(crate) fn push_returning<M: Model>(sql: &mut Sql) {
    sql.push(" RETURNING ");
    push_columns::<M>(sql);
}

#[doc(hidden)]
pub fn __insert_sql<M: Model>(records: &[M], upsert: bool) -> Sql {
    let conflict = upsert.then(|| Conflict {
        target: ConflictTarget::Columns(M::PRIMARY_KEYS.to_vec()),
        action: ConflictAction::UpdateAll,
    });
    insert_sql(records, upsert, conflict.as_ref()).expect("valid insert")
}
