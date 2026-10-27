//! `INSERT INTO … SELECT …`: rows written from a query's results, in the
//! database, without fetching them.

use std::fmt;

use crate::exec;
use crate::model::push_returning;
use crate::sql::Sql;
use crate::{Column, Error, Executor, Model, Result};

use super::Projected;

/// `INSERT INTO M (columns) SELECT …`, made by
/// [`Projected::insert_into`]: the selected values, in order, fill
/// `columns`.
///
/// ```ignore
/// let archived = Post::filter(Post::VIEWS.lt(10))
///     .select((Post::TITLE, Post::VIEWS))
///     .insert_into::<Archive>([Archive::TITLE, Archive::VIEWS])
///     .exec(&db)
///     .await?;
/// ```
pub struct InsertSelect<M, N> {
    columns: Vec<Column<M>>,
    select: Projected<N>,
}

impl<N: Model> Projected<N> {
    /// Insert this query's rows into `M`, its selected values filling
    /// `columns` in order; nothing is fetched. A different number of
    /// columns and selected values is an [`Error::InvalidQuery`] when run.
    pub fn insert_into<M: Model>(
        self,
        columns: impl IntoIterator<Item = Column<M>>,
    ) -> InsertSelect<M, N> {
        InsertSelect {
            columns: columns.into_iter().collect(),
            select: self,
        }
    }
}

impl<M: Model, N: Model> InsertSelect<M, N> {
    /// Render the statement (without `RETURNING`).
    pub fn to_sql(&self) -> Sql {
        let mut sql = Sql::new();
        sql.push("INSERT INTO ")
            .push_ident(M::TABLE)
            .push(" (")
            .push_list(&self.columns, ", ", |sql, column| {
                sql.push_ident(column.name());
            })
            .push(") ")
            .append(self.select.to_sql());
        sql
    }

    fn check(&self) -> Result<()> {
        if self.columns.len() == self.select.items.len() && !self.columns.is_empty() {
            Ok(())
        } else {
            Err(Error::InvalidQuery(format!(
                "`insert_into` `{}` names {} column(s) for {} selected value(s)",
                M::TABLE,
                self.columns.len(),
                self.select.items.len()
            )))
        }
    }

    /// Run it and return the number of rows inserted.
    pub async fn exec<'e, E: Executor<'e>>(self, executor: E) -> Result<u64> {
        self.check()?;
        exec::execute(executor, &self.to_sql(), &[M::TABLE]).await
    }

    /// Run it and return the inserted rows.
    pub async fn returning<'e, E: Executor<'e>>(self, executor: E) -> Result<Vec<M>> {
        self.check()?;
        let mut sql = self.to_sql();
        push_returning::<M>(&mut sql);
        exec::fetch_all(executor, &sql, &[M::TABLE], false).await
    }
}

impl<M: Model, N: Model> fmt::Debug for InsertSelect<M, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("InsertSelect").field(&self.to_sql()).finish()
    }
}
