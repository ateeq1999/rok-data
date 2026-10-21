//! The `Select` builder and the SQL it writes.
use std::fmt;
use std::marker::PhantomData;
use std::time::Duration;

use crate::cursor::Cursor;
use crate::expr::{Cond, IntoProjections, Projection};
use crate::model::push_columns;
use crate::sql::Sql;
use crate::{Column, Expr, Model, Order, Result};

use super::{Memoized, Projected, Update};
use super::{Trashed, implicit, push_where};

/// One leg of a `with`-loaded query on the session's [`Journey`]: every
/// arm re-borrows the path, so the rows and each include's batched load
pub(crate) mod exec;

/// The windowed read: [`Select::chunk`].
pub(crate) mod chunk;

/// Lock mode appended to a `SELECT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lock {
    Update,
    Share,
}
/// A `SELECT` query over model `M`, created with [`Model::query`] or [`Model::filter`].
///
/// Builders are cheap, cloneable values: nothing touches the database until
/// one of the async terminal methods (`all`, `first`, `count`, …) is awaited.
pub struct Select<M> {
    pub(crate) filters: Vec<Cond>,
    pub(crate) order: Vec<Order<M>>,
    pub(crate) group_by: Vec<(&'static str, &'static str)>,
    pub(crate) having: Vec<Cond>,
    pub(crate) limit: Option<u64>,
    pub(crate) offset: Option<u64>,
    pub(crate) lock: Option<Lock>,
    pub(crate) trashed: Trashed,
    pub(crate) scoped: bool,
    pub(crate) primary: bool,
    pub(crate) all_tenants: bool,
    pub(crate) joins: Vec<JoinClause>,
    pub(crate) _model: PhantomData<fn() -> M>,
}

/// One `JOIN` of a [`Joined`](crate::Joined) query.
#[derive(Debug, Clone)]
pub(crate) struct JoinClause {
    pub(crate) left: bool,
    pub(crate) table: &'static str,
    pub(crate) on: Cond,
    /// The joined model's implicit conditions (tenant, soft delete, default
    /// scope), evaluated at render time.
    pub(crate) scope: fn() -> Vec<Cond>,
    /// Whether this join can produce several rows per root row.
    pub(crate) multiplies: bool,
}

/// The implicit conditions of a joined model `N`.
pub(crate) fn join_scope<N: Model>() -> Vec<Cond> {
    implicit::<N>(Trashed::Exclude, true, false)
}

impl<M: Model> Default for Select<M> {
    fn default() -> Self {
        Self::new()
    }
}
impl<M: Model> Select<M> {
    /// A `SELECT` over every row of `M`'s table.
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
            order: Vec::new(),
            group_by: Vec::new(),
            having: Vec::new(),
            limit: None,
            offset: None,
            lock: None,
            trashed: Trashed::Exclude,
            scoped: true,
            primary: false,
            all_tenants: false,
            joins: Vec::new(),
            _model: PhantomData,
        }
    }

    /// Add a `WHERE` condition. Multiple calls are combined with `AND`.
    pub fn filter(mut self, expr: Expr<M>) -> Self {
        self.filters.push(expr.cond);
        self
    }

    /// Add a `WHERE` condition — the Eloquent spelling of
    /// [`filter`](Self::filter).
    ///
    /// `where` is a Rust keyword, so only the raw identifier `r#where`
    /// compiles; `filter` stays the canonical verb.
    ///
    /// ```ignore
    /// let admins = User::query()
    ///     .r#where(User::ROLE.eq("admin"))
    ///     .all(&db)
    ///     .await?;
    /// ```
    pub fn r#where(self, expr: Expr<M>) -> Self {
        self.filter(expr)
    }

    /// Query every tenant's rows of a `#[rok(tenant)]` model, ignoring the
    /// current tenant scope (for admin tools and background jobs).
    pub fn all_tenants(mut self) -> Self {
        self.all_tenants = true;
        self
    }

    /// Run on the primary even when read replicas are configured, e.g. to
    /// read your own writes without replication lag.
    pub fn on_primary(mut self) -> Self {
        self.primary = true;
        self
    }

    /// Whether this read may go to a replica: not forced to the primary and
    /// not locking rows.
    pub(crate) fn use_replica(&self) -> bool {
        !self.primary && self.lock.is_none()
    }

    /// Apply a reusable query fragment ("named scope"):
    ///
    /// ```ignore
    /// fn active(q: Select<User>) -> Select<User> { q.filter(User::ACTIVE.eq(true)) }
    /// fn newest(q: Select<User>) -> Select<User> { q.order_by(User::ID.desc()) }
    ///
    /// User::query().scope(active).scope(newest).limit(10)
    /// ```
    pub fn scope(self, scope: impl FnOnce(Self) -> Self) -> Self {
        scope(self)
    }

    /// Drop the model's default scope (`#[rok(default_scope = …)]`) for this
    /// query. Soft-delete filtering is controlled separately with
    /// [`with_trashed`](Self::with_trashed).
    pub fn unscoped(mut self) -> Self {
        self.scoped = false;
        self
    }

    /// Include soft-deleted rows (models with `#[rok(soft_delete)]` exclude
    /// them by default). No effect on other models.
    pub fn with_trashed(mut self) -> Self {
        self.trashed = Trashed::Include;
        self
    }

    /// Only soft-deleted rows. No effect on models without soft deletes.
    pub fn only_trashed(mut self) -> Self {
        self.trashed = Trashed::Only;
        self
    }

    /// Add a condition only when `condition` is true.
    pub fn filter_if(self, condition: bool, expr: impl FnOnce() -> Expr<M>) -> Self {
        if condition { self.filter(expr()) } else { self }
    }

    /// Add a condition built from an optional value, e.g. a search parameter.
    ///
    /// ```ignore
    /// User::query().filter_opt(params.role, |r| User::ROLE.eq(r))
    /// ```
    pub fn filter_opt<T>(self, value: Option<T>, expr: impl FnOnce(T) -> Expr<M>) -> Self {
        match value {
            Some(v) => self.filter(expr(v)),
            None => self,
        }
    }

    /// Append an `ORDER BY` term. Pass a column for ascending order, or
    /// `column.desc()`.
    pub fn order_by(mut self, order: impl Into<Order<M>>) -> Self {
        self.order.push(order.into());
        self
    }

    /// Append a `GROUP BY` column; combine with [`select`](Self::select).
    pub fn group_by(mut self, column: Column<M>) -> Self {
        self.group_by.push((M::TABLE, column.name()));
        self
    }

    /// Add a `HAVING` condition, typically on an aggregate
    /// (`User::AGE.avg().gt(30)`). Multiple calls are combined with `AND`.
    pub fn having(mut self, expr: Expr<M>) -> Self {
        self.having.push(expr.cond);
        self
    }

    /// Limit the number of returned rows.
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Skip the first `offset` rows.
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Lock selected rows with `FOR UPDATE` (use inside a transaction).
    pub fn for_update(mut self) -> Self {
        self.lock = Some(Lock::Update);
        self
    }

    /// Lock selected rows with `FOR SHARE` (use inside a transaction).
    pub fn for_share(mut self) -> Self {
        self.lock = Some(Lock::Share);
        self
    }

    /// Select specific columns or aggregates instead of whole models.
    ///
    /// ```ignore
    /// let emails: Vec<(i64, String)> = User::query()
    ///     .select((User::ID, User::EMAIL))
    ///     .fetch_all(&db)
    ///     .await?;
    /// ```
    pub fn select(self, projections: impl IntoProjections<M>) -> Projected<M> {
        Projected {
            select: self,
            items: projections.into_projections(),
        }
    }

    /// Cache this query's results for `ttl` in the pool's
    /// [`QueryCache`](crate::QueryCache).
    ///
    /// Writes made through rok-data invalidate the cached results of the
    /// table. When the executor has no cache (it wasn't enabled with
    /// [`DbBuilder::query_cache`](crate::DbBuilder::query_cache), or a plain
    /// sqlx pool or connection is used) the query simply runs every time.
    pub fn memoize(self, ttl: Duration) -> Memoized<M> {
        Memoized { select: self, ttl }
    }

    /// Turn this query into a bulk `UPDATE` with the same filters.
    pub fn update(self) -> Update<M> {
        Update {
            filters: self.filters,
            sets: Vec::new(),
            trashed: self.trashed,
            scoped: self.scoped,
            all_tenants: self.all_tenants,
            _model: PhantomData,
        }
    }

    /// Render the `SELECT` statement.
    pub fn to_sql(&self) -> Sql {
        let mut sql = self.new_sql();
        if self.multiplies() {
            self.write_distinct_rows(&mut sql, "", self.limit, self.offset);
            return sql;
        }
        sql.push("SELECT ");
        push_columns::<M>(&mut sql);
        self.write_tail(&mut sql);
        sql
    }

    /// A statement buffer; columns are table-qualified when there are joins.
    pub(crate) fn new_sql(&self) -> Sql {
        let mut sql = Sql::new();
        sql.qualify = !self.joins.is_empty();
        sql
    }

    /// Whether fetching root rows needs de-duplication: a join may produce
    /// several rows per root row and there is no `GROUP BY`.
    fn multiplies(&self) -> bool {
        self.group_by.is_empty() && self.joins.iter().any(|j| j.multiplies)
    }

    fn write_primary_keys(sql: &mut Sql) {
        sql.push_list(M::PRIMARY_KEYS, ", ", |sql, c| {
            sql.push_column(M::TABLE, c);
        });
    }

    /// One row per root record for a query whose joins may multiply rows:
    ///
    /// ```sql
    /// SELECT <head> d.* FROM (
    ///     SELECT DISTINCT ON (pk) <columns>, <order keys> AS __rok_o{i}
    ///     FROM … WHERE … ORDER BY pk, <order>
    /// ) d ORDER BY d.__rok_o{i} … LIMIT … OFFSET …
    /// ```
    fn write_distinct_rows(
        &self,
        sql: &mut Sql,
        head: &str,
        limit: Option<u64>,
        offset: Option<u64>,
    ) {
        sql.push("SELECT ")
            .push(head)
            .push(r#""__rok_d".* FROM (SELECT DISTINCT ON ("#);
        Self::write_primary_keys(sql);
        sql.push(") ");
        push_columns::<M>(sql);
        self.write_order_aliases(sql);
        self.write_from_where(sql);
        sql.push(" ORDER BY ");
        Self::write_primary_keys(sql);
        for o in &self.order {
            sql.push(", ");
            o.write(sql);
        }
        sql.push(r#") "__rok_d""#);
        self.write_alias_order(sql, r#""__rok_d"."#);
        if let Some(limit) = limit {
            sql.push(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = offset {
            sql.push(&format!(" OFFSET {offset}"));
        }
    }

    /// `, <order key> AS "__rok_o{i}"` for every order term.
    fn write_order_aliases(&self, sql: &mut Sql) {
        for (i, o) in self.order.iter().enumerate() {
            sql.push(", ");
            o.write_key(sql, "", None);
            sql.push(" AS ").push_ident(&format!("__rok_o{i}"));
        }
    }

    /// ` ORDER BY <prefix>"__rok_o{i}" …` for every order term.
    fn write_alias_order(&self, sql: &mut Sql, prefix: &str) {
        if !self.order.is_empty() {
            sql.push(" ORDER BY ")
                .push_list(self.order.iter().enumerate(), ", ", |sql, (i, o)| {
                    sql.push(prefix).push_ident(&format!("__rok_o{i}"));
                    o.write_direction(sql);
                });
        }
    }

    /// Everything after the select list.
    pub(crate) fn write_tail(&self, sql: &mut Sql) {
        self.write_from_where(sql);
        if !self.group_by.is_empty() {
            sql.push(" GROUP BY ")
                .push_list(&self.group_by, ", ", |sql, (t, c)| {
                    sql.push_column(t, c);
                });
        }
        push_where(sql, " HAVING ", &self.having);
        self.write_order(sql, "");
        if let Some(limit) = self.limit {
            sql.push(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = self.offset {
            sql.push(&format!(" OFFSET {offset}"));
        }
        match self.lock {
            Some(Lock::Update) => sql.push(" FOR UPDATE"),
            Some(Lock::Share) => sql.push(" FOR SHARE"),
            None => sql,
        };
    }

    fn write_from_where(&self, sql: &mut Sql) {
        sql.push(" FROM ").push_ident(M::TABLE);
        for join in &self.joins {
            sql.push(if join.left {
                " LEFT JOIN "
            } else {
                " INNER JOIN "
            })
            .push_ident(join.table)
            .push(" ON ");
            let scope = (join.scope)();
            sql.push_list(
                std::iter::once(&join.on).chain(&scope),
                " AND ",
                |sql, c| c.write(sql),
            );
        }
        let implicit = implicit::<M>(self.trashed, self.scoped, self.all_tenants);
        push_where(sql, " WHERE ", self.filters.iter().chain(&implicit));
    }

    /// `SELECT 1 FROM … WHERE …`, the body of an `EXISTS` subquery.
    pub(crate) fn write_exists_body(&self, sql: &mut Sql) {
        sql.push("SELECT 1");
        self.write_tail(sql);
    }

    /// `ORDER BY …`; with a `prefix` (the outer query of `paginate`),
    /// expression orders refer to their `__rok_o{i}` aliases.
    fn write_order(&self, sql: &mut Sql, prefix: &str) {
        if !self.order.is_empty() {
            sql.push(" ORDER BY ")
                .push_list(self.order.iter().enumerate(), ", ", |sql, (i, o)| {
                    let alias = (!prefix.is_empty()).then(|| format!("__rok_o{i}"));
                    o.write_key(sql, prefix, alias.as_deref());
                    o.write_direction(sql);
                });
        }
    }

    pub(crate) fn count_sql(&self) -> Sql {
        let mut sql = self.new_sql();
        self.write_count_head(&mut sql);
        self.write_from_where(&mut sql);
        sql
    }

    /// `SELECT COUNT(*)`, or `COUNT(DISTINCT pk)` when joins may multiply rows.
    fn write_count_head(&self, sql: &mut Sql) {
        if self.multiplies() {
            sql.push("SELECT COUNT(DISTINCT ");
            if M::PRIMARY_KEYS.len() > 1 {
                sql.push("(");
                Self::write_primary_keys(sql);
                sql.push(")");
            } else {
                Self::write_primary_keys(sql);
            }
            sql.push(")");
        } else {
            sql.push("SELECT COUNT(*)");
        }
    }

    pub(crate) fn exists_sql(&self) -> Sql {
        let mut sql = self.new_sql();
        sql.push("SELECT EXISTS(SELECT 1");
        self.write_from_where(&mut sql);
        sql.push(")");
        sql
    }

    fn aggregate_sql(&self, projection: Projection<M>) -> Sql {
        let mut sql = Sql::new();
        sql.push("SELECT ");
        projection.write(&mut sql);
        self.write_from_where(&mut sql);
        sql
    }
}
impl<M> Clone for Select<M> {
    fn clone(&self) -> Self {
        Self {
            filters: self.filters.clone(),
            order: self.order.clone(),
            group_by: self.group_by.clone(),
            having: self.having.clone(),
            limit: self.limit,
            offset: self.offset,
            lock: self.lock,
            trashed: self.trashed,
            scoped: self.scoped,
            primary: self.primary,
            all_tenants: self.all_tenants,
            joins: self.joins.clone(),
            _model: PhantomData,
        }
    }
}

impl<M: Model> fmt::Debug for Select<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Select").field(&self.to_sql()).finish()
    }
}
#[doc(hidden)]
pub fn __paginate_sql<M: Model>(select: &Select<M>, page: u64, per_page: u64) -> Sql {
    select.paginate_sql(page, per_page)
}

#[doc(hidden)]
pub fn __cursor_sql<M: Model>(
    select: Select<M>,
    after: Option<&Cursor>,
    limit: u64,
) -> Result<Sql> {
    if select.needs_wrapped_keyset() {
        return Ok(select.wrapped_cursor_sql(after, limit)?.0);
    }
    Ok(select.cursor_select(after, limit)?.to_sql())
}
