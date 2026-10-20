//! The single-row write: the `UPDATE`/`DELETE` with a version check.

use sqlx::Row;

use super::insert::push_columns;
use super::row::Model;
use crate::exec;
use crate::query::Select;
use crate::sql::Sql;
use crate::{Error, Executor, Result};

/// Run a single-row write (`UPDATE`/`DELETE` by primary key with a version
/// check) and tell apart success, a version conflict and a missing row in
/// one round trip:
///
/// ```sql
/// WITH w AS (<write> RETURNING TRUE AS __rok_present, <columns>)
/// SELECT EXISTS(<exists>) AS __rok_exists, w.*
/// FROM (SELECT 1) d LEFT JOIN w ON TRUE
/// ```
///
/// `exists` selects the record by primary key (without the version check).
pub(crate) async fn checked_write<'e, M: Model, E: Executor<'e>>(
    executor: E,
    write: Sql,
    key: String,
    exists: Select<M>,
) -> Result<M> {
    let mut sql = Sql::new();
    sql.push(r#"WITH "__rok_w" AS ("#);
    sql.append(write);
    sql.push(r#" RETURNING TRUE AS "__rok_present", "#);
    push_columns::<M>(&mut sql);
    sql.push(") SELECT EXISTS(");
    exists.write_exists_body(&mut sql);
    sql.push(r#") AS "__rok_exists", "w".* FROM (SELECT 1) AS "__rok_d" LEFT JOIN "__rok_w" AS "w" ON TRUE"#);

    let rows = exec::fetch_rows(executor, &sql, &[M::TABLE], false).await?;
    let row = rows
        .first()
        .ok_or_else(|| Error::not_found::<M>(Some(key.clone())))?;
    if row.try_get::<Option<bool>, _>("__rok_present")?.is_some() {
        Ok(M::from_row(row)?)
    } else if row.try_get::<bool, _>("__rok_exists")? {
        Err(Error::Conflict {
            table: M::TABLE,
            key,
        })
    } else {
        Err(Error::not_found::<M>(Some(key)))
    }
}
