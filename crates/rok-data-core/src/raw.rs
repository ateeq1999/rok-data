use sqlx::postgres::PgRow;
use sqlx::{Decode, FromRow, Postgres, Type};

use futures_core::stream::BoxStream;

use crate::exec;
use crate::sql::Sql;
use crate::{Error, Executor, Page, Result, Value};

/// Start a raw SQL query — the escape hatch for anything the query builder
/// doesn't express.
///
/// Placeholders are either `?` (numbered for you, in bind order) or
/// PostgreSQL's native `$1, $2, …` — use one style per query. Use `$n`
/// when your SQL contains the JSONB `?` operator.
///
/// ```ignore
/// let rows: Vec<User> = rok_data::raw("SELECT * FROM users WHERE age > ? AND name ILIKE ?")
///     .bind(18)
///     .bind("a%")
///     .fetch_all(&db)
///     .await?;
/// ```
pub fn raw(sql: impl Into<String>) -> Raw {
    Raw {
        sql: sql.into(),
        params: Vec::new(),
        writes: Vec::new(),
        replica: false,
        total: None,
    }
}

/// A raw SQL query with bound parameters, created with [`raw`].
#[derive(Debug, Clone)]
pub struct Raw {
    sql: String,
    params: Vec<Value>,
    writes: Vec<&'static str>,
    replica: bool,
    /// What [`fetch_page`](Raw::fetch_page) counts its total with, when
    /// not the statement itself.
    total: Option<String>,
}

impl Raw {
    /// Bind the next parameter.
    pub fn bind(mut self, value: impl Into<Value>) -> Self {
        self.params.push(value.into());
        self
    }

    /// Allow this read-only statement to run on a read replica. Raw SQL
    /// runs on the primary by default, since rok-data can't tell whether it
    /// writes.
    pub fn on_replica(mut self) -> Self {
        self.replica = true;
        self
    }

    /// Declare that this statement modifies `table`, so cached results of
    /// that table are invalidated when it succeeds (see
    /// [`QueryCache`](crate::QueryCache)).
    pub fn invalidates(mut self, table: &'static str) -> Self {
        self.writes.push(table);
        self
    }

    /// Render the statement with numbered placeholders.
    pub fn to_sql(&self) -> Sql {
        let mut sql = Sql::new();
        sql.push_raw_params(&self.sql, &self.params);
        sql
    }

    /// Fetch every row, decoded into `T` (any `FromRow` type, including models).
    pub async fn fetch_all<'e, T, E>(self, executor: E) -> Result<Vec<T>>
    where
        T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
        E: Executor<'e>,
    {
        exec::fetch_all(executor, &self.to_sql(), &self.writes, self.replica).await
    }

    /// Fetch at most one row.
    pub async fn fetch_optional<'e, T, E>(self, executor: E) -> Result<Option<T>>
    where
        T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
        E: Executor<'e>,
    {
        exec::fetch_optional(executor, &self.to_sql(), &self.writes, self.replica).await
    }

    /// Fetch exactly one row.
    pub async fn fetch_one<'e, T, E>(self, executor: E) -> Result<T>
    where
        T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
        E: Executor<'e>,
    {
        exec::fetch_optional(executor, &self.to_sql(), &self.writes, self.replica)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound.into())
    }

    /// Fetch the first column of the first row, e.g. `SELECT COUNT(*) …`.
    pub async fn scalar<'e, T, E>(self, executor: E) -> Result<T>
    where
        T: Type<Postgres> + for<'r> Decode<'r, Postgres> + Send + Unpin,
        E: Executor<'e>,
    {
        exec::fetch_scalar(executor, &self.to_sql(), self.replica).await
    }

    /// Fetch the first column of the first row, or `None` when there is no
    /// row, e.g. `DELETE … RETURNING key`.
    pub async fn scalar_optional<'e, T, E>(self, executor: E) -> Result<Option<T>>
    where
        T: Type<Postgres> + for<'r> Decode<'r, Postgres> + Send + Unpin,
        E: Executor<'e>,
    {
        let row: Option<(T,)> = self.fetch_optional(executor).await?;
        Ok(row.map(|(value,)| value))
    }

    /// Fetch the first column of every row, e.g. `SELECT id FROM …`.
    pub async fn scalars<'e, T, E>(self, executor: E) -> Result<Vec<T>>
    where
        T: Type<Postgres> + for<'r> Decode<'r, Postgres> + Send + Unpin,
        E: Executor<'e>,
    {
        let rows: Vec<(T,)> = self.fetch_all(executor).await?;
        Ok(rows.into_iter().map(|(value,)| value).collect())
    }

    /// Stream rows one at a time.
    pub fn stream<'e, T, E>(self, executor: E) -> BoxStream<'e, Result<T>>
    where
        T: for<'r> FromRow<'r, PgRow> + Send + Unpin + 'e,
        E: Executor<'e> + 'e,
    {
        exec::stream(executor, self.to_sql(), self.replica)
    }

    /// Count [`fetch_page`](Self::fetch_page)'s total with `sql`, a
    /// `SELECT count(*)` taking the same parameters, instead of counting
    /// the whole statement: typically the statement without its `ORDER BY`,
    /// which doesn't change a count.
    pub fn total_from(mut self, sql: impl Into<String>) -> Self {
        self.total = Some(sql.into());
        self
    }

    /// One page of the rows, with the total across every page: the
    /// statement with `LIMIT`/`OFFSET` appended, and
    /// `SELECT count(*) FROM (<statement>) q` (or [`total_from`](Self::total_from)'s
    /// count) bound with the same parameters. The two run one after the
    /// other on one connection of `conn`: `&db`, `&pool`, `&mut *tx` or
    /// `&mut conn`.
    ///
    /// `page` is 1-based (0 is 1). Past the last page, the page is empty
    /// and still reports the total. Write the statement with its
    /// `ORDER BY` and without `LIMIT`/`OFFSET`.
    ///
    /// ```ignore
    /// let page: Page<Booking> = rok_data::raw("SELECT * FROM bookings WHERE partner_id = $1 ORDER BY starts_on DESC")
    ///     .bind(partner)
    ///     .fetch_page(&db, 2, 20)
    ///     .await?;
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::InvalidQuery`](crate::Error::InvalidQuery) when `per_page`
    /// is 0, or the database's error.
    pub async fn fetch_page<'c, T, A>(self, conn: A, page: u64, per_page: u64) -> Result<Page<T>>
    where
        T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
        A: sqlx::Acquire<'c, Database = Postgres>,
    {
        if per_page == 0 {
            return Err(Error::InvalidQuery(
                "`per_page` must be greater than 0".into(),
            ));
        }
        let page = page.max(1);
        let limit = i64::try_from(per_page).unwrap_or(i64::MAX);
        let offset = i64::try_from((page - 1).saturating_mul(per_page)).unwrap_or(i64::MAX);

        // On their own lines, so a trailing `--` comment ends before them.
        let count = Raw {
            sql: self
                .total
                .clone()
                .unwrap_or_else(|| format!("SELECT count(*) FROM (\n{}\n) q", self.sql)),
            params: self.params.clone(),
            ..raw("")
        };
        let next = self.params.len() + 1;
        let paging = if self.sql.contains('?') {
            "\nLIMIT ? OFFSET ?".to_owned()
        } else {
            format!("\nLIMIT ${next} OFFSET ${}", next + 1)
        };
        let rows = Raw {
            sql: format!("{}{paging}", self.sql),
            total: None,
            ..self
        }
        .bind(limit)
        .bind(offset);

        let mut conn = conn.acquire().await?;
        let total: i64 = count.scalar(&mut *conn).await?;
        let items = rows.fetch_all(&mut *conn).await?;
        Ok(Page::new(
            items,
            u64::try_from(total).unwrap_or(0),
            page,
            per_page,
        ))
    }

    /// Execute the statement and return the number of affected rows.
    pub async fn execute<'e, E: Executor<'e>>(self, executor: E) -> Result<u64> {
        exec::execute(executor, &self.to_sql(), &self.writes).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_styles() {
        let q = raw("SELECT * FROM t WHERE a = ? AND b = ?")
            .bind(1)
            .bind("x");
        assert_eq!(
            q.to_sql().as_str(),
            "SELECT * FROM t WHERE a = $1 AND b = $2"
        );
        let q = raw("SELECT * FROM t WHERE a = $1").bind(1);
        assert_eq!(q.to_sql().as_str(), "SELECT * FROM t WHERE a = $1");
        assert_eq!(q.to_sql().params().len(), 1);
    }
}
