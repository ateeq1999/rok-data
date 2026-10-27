//! Many-to-many through a pivot table: a user has many roles and a role
//! many users, joined by `role_user (user_id, role_id)`.

use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;

use sqlx::postgres::PgRow;

use crate::expr::Cond;
use crate::query::JoinClause;
use crate::sql::{Sql, push_ident, qualified};
use crate::{Column, Executor, Expr, Model, Projection, Result, Select, Value};

use super::include::Relation;
use super::loaded::Many;
use super::relations::{key, require_single_key};

/// `P` has many `C` and `C` many `P`, through the rows of a pivot table:
/// `pivot.foreign_key → P.pk` and `pivot.related_key → C.pk`.
///
/// Generated as an associated constant by
/// `#[rok(belongs_to_many(roles = Role, pivot = "role_user"))]` on `P`
/// (`User::ROLES`); the pivot's columns default to `user_id` and
/// `role_id`, and `foreign_key = "…"` / `related_key = "…"` name others.
///
/// Besides loading and filtering like any relation, it writes the pivot:
///
/// ```ignore
/// User::ROLES.attach(&db, &user, [admin.id, editor.id]).await?;
/// User::ROLES.sync(&db, &user, [editor.id]).await?;   // only editor now
/// User::ROLES.toggle(&db, &user, [admin.id]).await?;  // admin back
/// User::ROLES.detach(&db, &user, [editor.id]).await?;
/// ```
pub struct BelongsToMany<P, C> {
    pivot: &'static str,
    foreign_key: &'static str,
    related_key: &'static str,
    _models: PhantomData<fn() -> (P, C)>,
}

impl<P, C> Clone for BelongsToMany<P, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P, C> Copy for BelongsToMany<P, C> {}

impl<P, C> fmt::Debug for BelongsToMany<P, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BelongsToMany")
            .field("pivot", &self.pivot)
            .field("foreign_key", &self.foreign_key)
            .field("related_key", &self.related_key)
            .finish()
    }
}

/// `$1, $2, …` for `values`, bound in order.
fn push_values(sql: &mut Sql, values: &[Value]) {
    sql.push_list(values, ", ", |sql, v| {
        sql.bind(v.clone());
    });
}

fn collect<V: Into<Value>>(ids: impl IntoIterator<Item = V>) -> Vec<Value> {
    ids.into_iter().map(Into::into).collect()
}

impl<P: Model, C: Model> BelongsToMany<P, C> {
    /// Declare the relation: the pivot table, its column holding `P`'s key,
    /// and its column holding `C`'s key.
    pub const fn new(
        pivot: &'static str,
        foreign_key: &'static str,
        related_key: &'static str,
    ) -> Self {
        Self {
            pivot,
            foreign_key,
            related_key,
            _models: PhantomData,
        }
    }

    /// The pivot table.
    pub const fn pivot(&self) -> &'static str {
        self.pivot
    }

    fn pivot_column(self, column: &str) -> String {
        qualified(self.pivot, column)
    }

    /// `C.pk IN (SELECT related_key FROM pivot WHERE <owner>)`, where
    /// `owner` is SQL over the pivot's `foreign_key` with its parameters.
    fn related_where(self, owner: &str, params: Vec<Value>) -> Expr<C> {
        let mut pivot = String::new();
        push_ident(&mut pivot, self.pivot);
        Expr::raw(
            format!(
                "{} IN (SELECT {} FROM {pivot} WHERE {owner})",
                qualified(C::TABLE, C::PRIMARY_KEY),
                self.pivot_column(self.related_key),
            ),
            params,
        )
    }

    /// Query the targets of `parent`.
    pub fn query(&self, parent: &P) -> Select<C> {
        let owner = format!("{} = ?", self.pivot_column(self.foreign_key));
        C::filter(self.related_where(&owner, vec![parent.primary_key()]))
    }

    /// `query` joined to the pivot: one row per (owner, target) pair.
    fn joined(self, mut base: Select<C>) -> Select<C> {
        base.joins.push(JoinClause {
            left: false,
            table: self.pivot,
            on: Cond::Raw {
                sql: format!(
                    "{} = {}",
                    self.pivot_column(self.related_key),
                    qualified(C::TABLE, C::PRIMARY_KEY)
                ),
                params: Vec::new(),
            },
            scope: Vec::new,
            multiplies: true,
        });
        base
    }

    async fn write<'e, E: Executor<'e>>(self, executor: E, sql: Sql) -> Result<u64> {
        crate::exec::execute(executor, &sql, &[self.pivot]).await
    }

    fn pivot_ident(self) -> String {
        let mut pivot = String::new();
        push_ident(&mut pivot, self.pivot);
        pivot
    }

    /// `INSERT INTO pivot (foreign_key, related_key [, extra…]) VALUES …
    /// ON CONFLICT DO NOTHING`, one row per id.
    fn insert_sql(self, owner: &Value, ids: &[Value], extra: &[(&str, Value)]) -> Sql {
        let mut sql = Sql::new();
        sql.push("INSERT INTO ")
            .push(&self.pivot_ident())
            .push(" (");
        sql.push_ident(self.foreign_key)
            .push(", ")
            .push_ident(self.related_key);
        for (column, _) in extra {
            sql.push(", ").push_ident(column);
        }
        sql.push(") VALUES ");
        sql.push_list(ids, ", ", |sql, id| {
            sql.push("(")
                .bind(owner.clone())
                .push(", ")
                .bind(id.clone());
            for (_, value) in extra {
                sql.push(", ").bind(value.clone());
            }
            sql.push(")");
        });
        sql.push(" ON CONFLICT DO NOTHING");
        sql
    }

    /// Attach `ids` to `owner`: one pivot row each, skipping the ones
    /// already there (when the pivot has a unique key on the pair). The
    /// number of rows written.
    pub async fn attach<'e, E, V>(
        self,
        executor: E,
        owner: &P,
        ids: impl IntoIterator<Item = V>,
    ) -> Result<u64>
    where
        E: Executor<'e>,
        V: Into<Value>,
    {
        self.attach_with(executor, owner, ids, []).await
    }

    /// [`attach`](Self::attach), setting more pivot columns on each new row:
    /// `User::ROLES.attach_with(&db, &user, [admin.id], [("granted_by", me.id.into())])`.
    pub async fn attach_with<'e, E, V>(
        self,
        executor: E,
        owner: &P,
        ids: impl IntoIterator<Item = V>,
        pivot_values: impl IntoIterator<Item = (&'static str, Value)>,
    ) -> Result<u64>
    where
        E: Executor<'e>,
        V: Into<Value>,
    {
        let ids = collect(ids);
        if ids.is_empty() {
            return Ok(0);
        }
        let extra: Vec<(&str, Value)> = pivot_values.into_iter().collect();
        let sql = self.insert_sql(&owner.primary_key(), &ids, &extra);
        self.write(executor, sql).await
    }

    /// Detach `ids` from `owner`: their pivot rows are deleted. The number
    /// of rows deleted.
    pub async fn detach<'e, E, V>(
        self,
        executor: E,
        owner: &P,
        ids: impl IntoIterator<Item = V>,
    ) -> Result<u64>
    where
        E: Executor<'e>,
        V: Into<Value>,
    {
        let ids = collect(ids);
        if ids.is_empty() {
            return Ok(0);
        }
        let mut sql = self.delete_head(&owner.primary_key());
        sql.push(" AND ").push_ident(self.related_key).push(" IN (");
        push_values(&mut sql, &ids);
        sql.push(")");
        self.write(executor, sql).await
    }

    /// Detach everything from `owner`.
    pub async fn detach_all<'e, E: Executor<'e>>(self, executor: E, owner: &P) -> Result<u64> {
        let sql = self.delete_head(&owner.primary_key());
        self.write(executor, sql).await
    }

    fn delete_head(self, owner: &Value) -> Sql {
        let mut sql = Sql::new();
        sql.push("DELETE FROM ")
            .push(&self.pivot_ident())
            .push(" WHERE ")
            .push_ident(self.foreign_key)
            .push(" = ")
            .bind(owner.clone());
        sql
    }

    /// Make `ids` exactly `owner`'s targets: the others are detached and
    /// the missing ones attached, in one statement.
    pub async fn sync<'e, E, V>(
        self,
        executor: E,
        owner: &P,
        ids: impl IntoIterator<Item = V>,
    ) -> Result<()>
    where
        E: Executor<'e>,
        V: Into<Value>,
    {
        let ids = collect(ids);
        let owner = owner.primary_key();
        let mut sql = Sql::new();
        if ids.is_empty() {
            sql = self.delete_head(&owner);
        } else {
            sql.push("WITH \"__rok_gone\" AS (");
            sql.append(self.delete_head(&owner));
            sql.push(" AND ")
                .push_ident(self.related_key)
                .push(" NOT IN (");
            push_values(&mut sql, &ids);
            sql.push(")) ");
            sql.append(self.insert_sql(&owner, &ids, &[]));
        }
        self.write(executor, sql).await.map(|_| ())
    }

    /// Attach the `ids` not attached yet, keeping every other target.
    pub async fn sync_without_detaching<'e, E, V>(
        self,
        executor: E,
        owner: &P,
        ids: impl IntoIterator<Item = V>,
    ) -> Result<()>
    where
        E: Executor<'e>,
        V: Into<Value>,
    {
        self.attach(executor, owner, ids).await.map(|_| ())
    }

    /// Flip each of `ids`: attached ones are detached, the others attached,
    /// in one statement.
    pub async fn toggle<'e, E, V>(
        self,
        executor: E,
        owner: &P,
        ids: impl IntoIterator<Item = V>,
    ) -> Result<()>
    where
        E: Executor<'e>,
        V: Into<Value>,
    {
        let ids = collect(ids);
        if ids.is_empty() {
            return Ok(());
        }
        let owner = owner.primary_key();
        let mut sql = Sql::new();
        sql.push("WITH \"__rok_gone\" AS (");
        sql.append(self.delete_head(&owner));
        sql.push(" AND ").push_ident(self.related_key).push(" IN (");
        push_values(&mut sql, &ids);
        sql.push(") RETURNING ").push_ident(self.related_key);
        sql.push(") INSERT INTO ")
            .push(&self.pivot_ident())
            .push(" (");
        sql.push_ident(self.foreign_key)
            .push(", ")
            .push_ident(self.related_key);
        sql.push(") SELECT ")
            .bind(owner)
            .push(", \"__rok_v\".\"id\" FROM (VALUES ");
        sql.push_list(&ids, ", ", |sql, id| {
            sql.push("(").bind(id.clone()).push(")");
        });
        sql.push(") AS \"__rok_v\" (\"id\") WHERE \"__rok_v\".\"id\" NOT IN (SELECT ")
            .push_ident(self.related_key)
            .push(" FROM \"__rok_gone\") ON CONFLICT DO NOTHING");
        self.write(executor, sql).await.map(|_| ())
    }

    /// Set pivot columns on the row tying `owner` to `id`. The number of
    /// rows updated (0 when they aren't attached).
    pub async fn update_existing_pivot<'e, E, V>(
        self,
        executor: E,
        owner: &P,
        id: V,
        pivot_values: impl IntoIterator<Item = (&'static str, Value)>,
    ) -> Result<u64>
    where
        E: Executor<'e>,
        V: Into<Value>,
    {
        let values: Vec<(&str, Value)> = pivot_values.into_iter().collect();
        if values.is_empty() {
            return Ok(0);
        }
        let mut sql = Sql::new();
        sql.push("UPDATE ").push(&self.pivot_ident()).push(" SET ");
        sql.push_list(&values, ", ", |sql, (column, value)| {
            sql.push_ident(column).push(" = ").bind(value.clone());
        });
        sql.push(" WHERE ")
            .push_ident(self.foreign_key)
            .push(" = ")
            .bind(owner.primary_key())
            .push(" AND ")
            .push_ident(self.related_key)
            .push(" = ")
            .bind(id.into());
        self.write(executor, sql).await
    }
}

impl<P: Model, C: Model> Relation for BelongsToMany<P, C> {
    type Owner = P;
    type Target = C;
    type Loaded = Many<P, C>;
    type Read<'a> = &'a [C];

    fn foreign_key_name(self) -> &'static str {
        self.foreign_key
    }

    fn owner_key(self) -> Column<P> {
        P::primary_key_column()
    }

    /// The target's primary key, which the pivot's `related_key` holds.
    fn target_key(self) -> Column<C> {
        C::primary_key_column()
    }

    fn check(self) -> Result<()> {
        require_single_key::<P>()?;
        require_single_key::<C>()
    }

    fn assemble(self, targets: Vec<PgRow>) -> Result<Many<P, C>> {
        let mut map: HashMap<String, Vec<C>> = HashMap::new();
        for row in targets {
            let target = C::from_row(&row).map_err(crate::Error::from)?;
            let owner = Value::from_row_column(&row, "__rok_owner")?;
            if let Some(owner) = key(Some(owner)) {
                map.entry(owner).or_default().push(target);
            }
        }
        Ok(Many::new(map, P::PRIMARY_KEY))
    }

    fn read<'a>(self, state: &'a Many<P, C>, owner: &P) -> Option<&'a [C]> {
        Some(state.get(owner))
    }

    fn targets(self, state: &Many<P, C>) -> Vec<&C> {
        state.values().collect()
    }

    /// Over the pivot-joined batch: the pivot's `foreign_key` among `ids`.
    fn batch_filter(self, ids: Vec<Value>) -> Expr<C> {
        if ids.is_empty() {
            return Expr::none();
        }
        let placeholders = vec!["?"; ids.len()].join(", ");
        Expr::raw(
            format!(
                "{} IN ({placeholders})",
                self.pivot_column(self.foreign_key)
            ),
            ids,
        )
    }

    fn batch_base(self) -> Select<C> {
        self.joined(C::query())
    }

    /// One row per (owner, target) pair, its owner key as `__rok_owner`.
    fn batch_sql(self, base: Select<C>, ids: Vec<Value>) -> Sql {
        let mut items: Vec<Projection<C>> = C::COLUMNS
            .iter()
            .map(|c| Projection::from(Column::<C>::new(c)))
            .collect();
        items.push(self.owner_of_target().alias("__rok_owner"));
        self.joined(base)
            .filter(self.batch_filter(ids))
            .select(items)
            .to_sql()
    }

    fn correlate(self) -> Expr<C> {
        let owner = format!(
            "{} = {}",
            self.pivot_column(self.foreign_key),
            qualified(P::TABLE, P::PRIMARY_KEY)
        );
        self.related_where(&owner, Vec::new())
    }

    /// The joined pivot's `foreign_key`.
    fn owner_of_target(self) -> Projection<C> {
        Projection::raw::<Value>(self.pivot_column(self.foreign_key), [])
    }
}
