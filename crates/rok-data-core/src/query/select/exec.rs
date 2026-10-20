//! The terminal methods that run a `Select`: fetching, paginating, deleting.
use futures_core::stream::BoxStream;
use sqlx::{Decode, Postgres, Row, Type};

use crate::cursor::{Cursor, CursorPage};
use crate::exec::{self};
use crate::expr::{Cond, Direction};
use crate::model::{Model, push_columns};
use crate::relation::IntoInclude;
use crate::sql::Sql;
use crate::{Column, Error, Executor, Expr, Order, Page, Result, Value};

use super::Select;
use crate::query::WithSelect;

impl<M: Model> Select<M> {
    /// Loads a relation beside the rows: the batched follow-up query runs
    /// once for the whole match (defeating N+1), and terminals return a
    /// [`Loaded`](crate::relation::Loaded) carrying the rows and their
    /// loaded relations.
    ///
    /// Accepted as the include: a relation constant (`User::POSTS`), or
    /// an [`Include`](crate::relation::Include) with filters, ordering,
    /// limit and offset on the batch.
    ///
    /// ```ignore
    /// let view = User::filter(User::ACTIVE.eq(true))
    ///     .with(User::POSTS)
    ///     .all(&db)
    ///     .await?;
    /// for user in view.rows() {
    ///     let posts = view.try_get(User::POSTS, user);
    /// }
    /// ```
    pub fn with<I: IntoInclude<M>>(self, include: I) -> WithSelect<M> {
        WithSelect {
            select: self,
            includes: vec![include.into_packed()],
        }
    }

    /// Fetch every matching row.
    pub async fn all<'e, E: Executor<'e>>(self, executor: E) -> Result<Vec<M>> {
        exec::fetch_all(executor, &self.to_sql(), &[], self.use_replica()).await
    }

    /// Fetch the first matching row, or `None`.
    pub async fn first<'e, E: Executor<'e>>(self, executor: E) -> Result<Option<M>> {
        let replica = self.use_replica();
        exec::fetch_optional(executor, &self.limit(1).to_sql(), &[], replica).await
    }

    /// Fetch the first matching row, failing with [`Error::NotFound`].
    pub async fn one<'e, E: Executor<'e>>(self, executor: E) -> Result<M> {
        self.first(executor)
            .await?
            .ok_or_else(|| Error::not_found::<M>(None))
    }

    /// Stream matching rows one at a time instead of loading them all into
    /// memory. Consume it with `futures::TryStreamExt` (re-exported in the
    /// rok-data prelude):
    ///
    /// ```ignore
    /// let mut users = User::query().stream(&db);
    /// while let Some(user) = users.try_next().await? {
    ///     // …
    /// }
    /// ```
    pub fn stream<'e, E: Executor<'e> + 'e>(self, executor: E) -> BoxStream<'e, Result<M>> {
        let replica = self.use_replica();
        exec::stream(executor, self.to_sql(), replica)
    }

    /// Count the matching rows (ignores ordering, limit and offset).
    pub async fn count<'e, E: Executor<'e>>(self, executor: E) -> Result<i64> {
        exec::fetch_scalar(executor, &self.count_sql(), self.use_replica()).await
    }

    /// `true` if at least one row matches.
    pub async fn exists<'e, E: Executor<'e>>(self, executor: E) -> Result<bool> {
        exec::fetch_scalar(executor, &self.exists_sql(), self.use_replica()).await
    }

    /// `SUM(column)` over the matching rows as `BIGINT` (`None` when no
    /// rows match). Use [`select`](Self::select) with
    /// [`Projection::cast`] for other result types.
    pub async fn sum<'e, E: Executor<'e>>(
        self,
        executor: E,
        column: Column<M>,
    ) -> Result<Option<i64>> {
        let sql = self.aggregate_sql(column.sum().cast("BIGINT"));
        exec::fetch_scalar(executor, &sql, self.use_replica()).await
    }

    /// `AVG(column)` over the matching rows as `DOUBLE PRECISION`.
    pub async fn avg<'e, E: Executor<'e>>(
        self,
        executor: E,
        column: Column<M>,
    ) -> Result<Option<f64>> {
        let sql = self.aggregate_sql(column.avg().cast("DOUBLE PRECISION"));
        exec::fetch_scalar(executor, &sql, self.use_replica()).await
    }

    /// `MIN(column)` over the matching rows.
    pub async fn min<'e, T, E>(self, executor: E, column: Column<M>) -> Result<Option<T>>
    where
        T: Type<Postgres> + for<'r> Decode<'r, Postgres> + Send + Unpin,
        E: Executor<'e>,
    {
        exec::fetch_scalar(
            executor,
            &self.aggregate_sql(column.min()),
            self.use_replica(),
        )
        .await
    }

    /// `MAX(column)` over the matching rows.
    pub async fn max<'e, T, E>(self, executor: E, column: Column<M>) -> Result<Option<T>>
    where
        T: Type<Postgres> + for<'r> Decode<'r, Postgres> + Send + Unpin,
        E: Executor<'e>,
    {
        exec::fetch_scalar(
            executor,
            &self.aggregate_sql(column.max()),
            self.use_replica(),
        )
        .await
    }

    /// Fetch one page of results together with the total number of matches,
    /// in a single round trip. `page` is 1-based.
    pub async fn paginate<'e, E: Executor<'e>>(
        self,
        executor: E,
        page: u64,
        per_page: u64,
    ) -> Result<Page<M>> {
        if per_page == 0 {
            return Err(Error::InvalidQuery(
                "`per_page` must be greater than 0".into(),
            ));
        }
        let page = page.max(1);
        let rows = exec::fetch_rows(
            executor,
            &self.paginate_sql(page, per_page),
            &[],
            self.use_replica(),
        )
        .await?;

        let mut total = 0;
        let mut items = Vec::with_capacity(rows.len());
        for row in &rows {
            total = row.try_get::<i64, _>("__rok_total")?;
            if row.try_get::<Option<bool>, _>("__rok_present")?.is_some() {
                items.push(M::from_row(row)?);
            }
        }
        Ok(Page::new(items, total.max(0) as u64, page, per_page))
    }

    pub(crate) fn paginate_sql(&self, page: u64, per_page: u64) -> Sql {
        if !self.joins.is_empty() {
            return self.joined_paginate_sql(page, per_page);
        }
        // `count LEFT JOIN LATERAL (page)` always yields at least one row, so
        // the total is known even when the requested page is empty.
        let mut sql = Sql::new();
        sql.push(r#"SELECT c."__rok_total", p.* FROM (SELECT COUNT(*) AS "__rok_total""#);
        self.write_from_where(&mut sql);
        sql.push(r#") c LEFT JOIN LATERAL (SELECT TRUE AS "__rok_present", "#);
        push_columns::<M>(&mut sql);
        // Expression orders are selected so the outer query can sort by them.
        for (i, o) in self.order.iter().enumerate() {
            if o.column().is_none() {
                sql.push(", ");
                o.write_key(&mut sql, "", None);
                sql.push(" AS ").push_ident(&format!("__rok_o{i}"));
            }
        }
        self.write_from_where(&mut sql);
        self.write_order(&mut sql, "");
        let offset = (page - 1).saturating_mul(per_page);
        sql.push(&format!(" LIMIT {per_page} OFFSET {offset}) p ON TRUE"));
        self.write_order(&mut sql, "p.");
        sql
    }

    /// [`paginate_sql`](Self::paginate_sql) for queries with joins: every
    /// order key is selected under an alias so the outer query can sort by
    /// it, and multiplying joins are de-duplicated.
    pub(crate) fn joined_paginate_sql(&self, page: u64, per_page: u64) -> Sql {
        let mut sql = self.new_sql();
        sql.push(r#"SELECT c."__rok_total", p.* FROM ("#);
        self.write_count_head(&mut sql);
        sql.push(r#" AS "__rok_total""#);
        self.write_from_where(&mut sql);
        sql.push(") c LEFT JOIN LATERAL (");
        let offset = (page - 1).saturating_mul(per_page);
        let head = r#"TRUE AS "__rok_present", "#;
        if self.multiplies() {
            self.write_distinct_rows(&mut sql, head, Some(per_page), Some(offset));
        } else {
            sql.push("SELECT ").push(head);
            push_columns::<M>(&mut sql);
            self.write_order_aliases(&mut sql);
            self.write_from_where(&mut sql);
            self.write_order(&mut sql, "");
            sql.push(&format!(" LIMIT {per_page} OFFSET {offset}"));
        }
        sql.push(") p ON TRUE");
        self.write_alias_order(&mut sql, "p.");
        sql
    }

    /// Delete every matching row and return how many were deleted.
    ///
    /// For models with soft deletes this sets `deleted_at = now()` instead;
    /// use [`force_delete`](Self::force_delete) to remove rows for good.
    pub async fn delete<'e, E: Executor<'e>>(self, executor: E) -> Result<u64> {
        exec::execute(executor, &self.delete_sql(), &[M::TABLE]).await
    }

    /// Render the statement [`delete`](Self::delete) would run.
    pub fn delete_sql(&self) -> Sql {
        match M::DELETED_AT_COLUMN {
            Some(deleted_at) => self
                .clone()
                .update()
                .set_raw(Column::new(deleted_at), "now()", [] as [Value; 0])
                .to_sql(),
            None => self.force_delete_sql(),
        }
    }

    /// `DELETE` every matching row, even for models with soft deletes.
    /// Soft-deleted rows are only matched with
    /// [`with_trashed`](Self::with_trashed) or [`only_trashed`](Self::only_trashed).
    pub async fn force_delete<'e, E: Executor<'e>>(self, executor: E) -> Result<u64> {
        exec::execute(executor, &self.force_delete_sql(), &[M::TABLE]).await
    }

    pub(crate) fn force_delete_sql(&self) -> Sql {
        let mut sql = Sql::new();
        sql.push("DELETE");
        self.write_from_where(&mut sql);
        sql
    }

    /// Restore every matching soft-deleted row (`deleted_at = NULL`) and
    /// return how many were restored.
    pub async fn restore<'e, E: Executor<'e>>(self, executor: E) -> Result<u64> {
        let Some(deleted_at) = M::DELETED_AT_COLUMN else {
            return Err(Error::InvalidQuery(format!(
                "`{}` has no soft-delete column",
                M::TABLE
            )));
        };
        self.only_trashed()
            .update()
            .set_raw(Column::new(deleted_at), "NULL", [] as [Value; 0])
            .exec(executor)
            .await
    }

    /// Every table this query reads: the root and joined tables.
    pub(crate) fn tables(&self) -> Vec<&'static str> {
        std::iter::once(M::TABLE)
            .chain(self.joins.iter().map(|j| j.table))
            .collect()
    }

    /// A fresh `SELECT` of `M` restricted to the root rows this (joined)
    /// query matches: `pk IN (SELECT root.pk FROM … JOIN … WHERE …)`.
    /// Used for bulk writes through joins.
    pub(crate) fn key_subselect(&self) -> Select<M> {
        let inner = self.clone();
        let render = crate::expr::Render::new(move |sql: &mut Sql| {
            let previous = std::mem::replace(&mut sql.qualify, true);
            sql.push("SELECT ");
            Self::write_primary_keys(sql);
            inner.write_from_where(sql);
            sql.qualify = previous;
        });
        let kind = if M::PRIMARY_KEYS.len() == 1 {
            crate::expr::SubKind::In {
                table: M::TABLE,
                column: M::PRIMARY_KEY,
                negated: false,
            }
        } else {
            crate::expr::SubKind::InRow {
                table: M::TABLE,
                columns: M::PRIMARY_KEYS,
            }
        };
        Select {
            filters: vec![Cond::Sub { kind, render }],
            trashed: self.trashed,
            scoped: self.scoped,
            all_tenants: self.all_tenants,
            ..Select::new()
        }
    }

    /// The ordering used for keyset pagination: the query's `ORDER BY`
    /// columns plus the primary key as a tiebreaker.
    pub(crate) fn keyset_order(&self) -> Vec<(&'static str, Direction)> {
        let mut order: Vec<_> = self
            .order
            .iter()
            .filter_map(|o| o.column().map(|c| (c, o.direction)))
            .collect();
        for key in M::PRIMARY_KEYS {
            if !order.iter().any(|(c, _)| c == key) {
                order.push((key, Direction::Asc));
            }
        }
        order
    }

    /// Keyset ("cursor") pagination: fetch up to `limit` rows after
    /// `cursor`, following the query's `ORDER BY` (the primary key is added
    /// as a tiebreaker). Unlike [`paginate`](Self::paginate), the cost
    /// doesn't grow with the page number and rows aren't skipped or repeated
    /// when data changes between requests.
    ///
    /// Sort keys (columns or expressions) must be `NOT NULL`; expression
    /// orders and joined queries paginate over a wrapped subquery. Pass
    /// `None` for the first page,
    /// then `page.next` for the following ones:
    ///
    /// ```ignore
    /// let page = Post::order_by(Post::CREATED_AT.desc())
    ///     .cursor_paginate(&db, None, 20)
    ///     .await?;
    /// let token = page.next.map(|c| c.to_string()); // hand to the client
    ///
    /// let cursor: Cursor = token.unwrap().parse()?;
    /// let page2 = Post::order_by(Post::CREATED_AT.desc())
    ///     .cursor_paginate(&db, Some(&cursor), 20)
    ///     .await?;
    /// ```
    pub async fn cursor_paginate<'e, E: Executor<'e>>(
        self,
        executor: E,
        after: Option<&Cursor>,
        limit: u64,
    ) -> Result<CursorPage<M>> {
        if limit == 0 {
            return Err(Error::InvalidQuery("`limit` must be greater than 0".into()));
        }
        if self.needs_wrapped_keyset() {
            return self.wrapped_cursor_paginate(executor, after, limit).await;
        }
        let select = self.cursor_select(after, limit)?;
        let order = select.keyset_order();
        let mut items = select.all(executor).await?;
        let next = if items.len() as u64 > limit {
            items.truncate(limit as usize);
            let last = items.last().expect("limit > 0");
            let values = order
                .iter()
                .map(|(c, _)| {
                    last.value_of(c).ok_or_else(|| {
                        Error::InvalidQuery(format!("`{c}` is not a column of `{}`", M::TABLE))
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Some(Cursor::new(values))
        } else {
            None
        };
        Ok(CursorPage { items, next })
    }

    /// Joined queries and expression orders paginate over aliased sort keys.
    pub(crate) fn needs_wrapped_keyset(&self) -> bool {
        !self.joins.is_empty() || self.order.iter().any(|o| o.column().is_none())
    }

    /// The order with the root primary key appended as a tiebreaker.
    pub(crate) fn keyset_orders(&self) -> Vec<Order<M>> {
        let mut order = self.order.clone();
        for key in M::PRIMARY_KEYS {
            let present = order.iter().any(|o| {
                matches!(o.target, crate::expr::OrderTarget::Column { table, name } if table == M::TABLE && name == *key)
            });
            if !present {
                order.push(Column::<M>::new(key).asc());
            }
        }
        order
    }

    /// ```sql
    /// SELECT * FROM (
    ///     SELECT [DISTINCT ON (pk)] <columns>, <sort keys> AS __rok_o{i} FROM … WHERE …
    /// ) k WHERE (k.__rok_o0 > $ OR (k.__rok_o0 = $ AND k.__rok_o1 > $) …)
    /// ORDER BY k.__rok_o0, … LIMIT n + 1
    /// ```
    pub(crate) fn wrapped_cursor_sql(
        &self,
        after: Option<&Cursor>,
        limit: u64,
    ) -> Result<(Sql, usize)> {
        const ALIASES: [&str; 16] = [
            "__rok_o0",
            "__rok_o1",
            "__rok_o2",
            "__rok_o3",
            "__rok_o4",
            "__rok_o5",
            "__rok_o6",
            "__rok_o7",
            "__rok_o8",
            "__rok_o9",
            "__rok_o10",
            "__rok_o11",
            "__rok_o12",
            "__rok_o13",
            "__rok_o14",
            "__rok_o15",
        ];
        let mut keyed = self.clone();
        keyed.order = self.keyset_orders();
        let n = keyed.order.len();
        if n > ALIASES.len() {
            return Err(Error::InvalidQuery(
                "too many sort keys for keyset pagination".into(),
            ));
        }
        let mut sql = keyed.new_sql();
        sql.qualify = true;
        sql.push("SELECT * FROM (SELECT ");
        let multiplies = keyed.multiplies();
        if multiplies {
            sql.push("DISTINCT ON (");
            Self::write_primary_keys(&mut sql);
            sql.push(") ");
        }
        push_columns::<M>(&mut sql);
        keyed.write_order_aliases(&mut sql);
        keyed.write_from_where(&mut sql);
        if multiplies {
            sql.push(" ORDER BY ");
            Self::write_primary_keys(&mut sql);
            for o in &keyed.order {
                sql.push(", ");
                o.write(&mut sql);
            }
        }
        sql.push(r#") "__rok_k""#);
        let key = |i: usize| crate::expr::Term::Column {
            table: "__rok_k",
            name: ALIASES[i],
        };
        if let Some(cursor) = after {
            if cursor.values.len() != n {
                return Err(Error::InvalidCursor(format!(
                    "expected {n} values, got {}",
                    cursor.values.len()
                )));
            }
            let cmp = |i: usize, op: &'static str| Cond::Cmp {
                term: key(i),
                op,
                value: cursor.values[i].clone(),
            };
            let branches = (0..n)
                .map(|i| {
                    let step = match keyed.order[i].direction {
                        crate::expr::Direction::Asc => cmp(i, ">"),
                        crate::expr::Direction::Desc => cmp(i, "<"),
                    };
                    Cond::And((0..i).map(|j| cmp(j, "=")).chain([step]).collect())
                })
                .collect();
            sql.push(" WHERE ");
            Cond::Or(branches).write(&mut sql);
        }
        sql.push(" ORDER BY ").push_list(0..n, ", ", |sql, i| {
            key(i).write(sql);
            keyed.order[i].write_direction(sql);
        });
        sql.push(&format!(" LIMIT {}", limit + 1));
        Ok((sql, n))
    }

    async fn wrapped_cursor_paginate<'e, E: Executor<'e>>(
        self,
        executor: E,
        after: Option<&Cursor>,
        limit: u64,
    ) -> Result<CursorPage<M>> {
        let (sql, n) = self.wrapped_cursor_sql(after, limit)?;
        let rows = exec::fetch_rows(executor, &sql, &[], self.use_replica()).await?;
        let more = rows.len() as u64 > limit;
        let kept = &rows[..rows.len().min(limit as usize)];
        let items = kept
            .iter()
            .map(M::from_row)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let next = match kept.last() {
            Some(last) if more => Some(Cursor::new(
                (0..n)
                    .map(|i| Value::from_row_column(last, &format!("__rok_o{i}")))
                    .collect::<Result<Vec<_>>>()?,
            )),
            _ => None,
        };
        Ok(CursorPage { items, next })
    }

    pub(crate) fn cursor_select(self, after: Option<&Cursor>, limit: u64) -> Result<Self> {
        let order = self.keyset_order();
        let mut select = self;
        select.order = order
            .iter()
            .map(|(c, d)| match d {
                Direction::Asc => Column::new(c).asc(),
                Direction::Desc => Column::new(c).desc(),
            })
            .collect();
        select.offset = None;
        select.limit = Some(limit + 1);
        if let Some(cursor) = after {
            if cursor.values.len() != order.len() {
                return Err(Error::InvalidCursor(format!(
                    "expected {} values, got {}",
                    order.len(),
                    cursor.values.len()
                )));
            }
            // (a > x) OR (a = x AND b > y) OR … — works for mixed directions.
            let branches = (0..order.len()).map(|i| {
                let eqs = (0..i).map(|j| Column::<M>::new(order[j].0).eq(cursor.values[j].clone()));
                let (column, direction) = order[i];
                let value = cursor.values[i].clone();
                let step = match direction {
                    Direction::Asc => Column::<M>::new(column).gt(value),
                    Direction::Desc => Column::<M>::new(column).lt(value),
                };
                Expr::all_of(eqs.chain([step]))
            });
            select = select.filter(Expr::any_of(branches));
        }
        Ok(select)
    }
}
