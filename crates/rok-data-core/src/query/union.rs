//! `UNION` of two or more reads of one model.

use crate::exec;
use crate::sql::Sql;
use crate::{Executor, Model, ModelCollection, Order, Result};

use super::Select;

/// Two or more [`Select`]s of model `M` combined with `UNION` or
/// `UNION ALL`, made by [`Select::union`] or [`Select::union_all`].
///
/// Each leg keeps its own filters, scopes, order and limit. The order,
/// limit and offset set here apply to the combined rows; order by columns,
/// since a `UNION` sorts by its output columns only.
///
/// ```ignore
/// let rows = User::filter(User::ROLE.eq("admin"))
///     .union(User::filter(User::EMAIL.ends_with("@example.com")))
///     .order_by(User::EMAIL)
///     .limit(50)
///     .all(&db)
///     .await?;
/// ```
pub struct Union<M> {
    first: Select<M>,
    rest: Vec<(bool, Select<M>)>,
    order: Vec<Order<M>>,
    limit: Option<u64>,
    offset: Option<u64>,
}

impl<M: Model> Union<M> {
    pub(crate) fn new(first: Select<M>) -> Self {
        Self {
            first,
            rest: Vec::new(),
            order: Vec::new(),
            limit: None,
            offset: None,
        }
    }

    /// Add `UNION other`: its rows, without duplicates.
    pub fn union(mut self, other: Select<M>) -> Self {
        self.rest.push((false, other));
        self
    }

    /// Add `UNION ALL other`: its rows, duplicates kept.
    pub fn union_all(mut self, other: Select<M>) -> Self {
        self.rest.push((true, other));
        self
    }

    /// Append an `ORDER BY` term over the combined rows.
    pub fn order_by(mut self, order: impl Into<Order<M>>) -> Self {
        self.order.push(order.into());
        self
    }

    /// Limit the number of combined rows.
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Skip the first `offset` combined rows.
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Render the statement: each leg in parentheses, then the order,
    /// limit and offset of the whole.
    pub fn to_sql(&self) -> Sql {
        let mut sql = Sql::new();
        sql.push("(").append(self.first.to_sql()).push(")");
        for (all, leg) in &self.rest {
            sql.push(if *all { " UNION ALL (" } else { " UNION (" })
                .append(leg.to_sql())
                .push(")");
        }
        if !self.order.is_empty() {
            sql.push(" ORDER BY ")
                .push_list(&self.order, ", ", |sql, order| order.write(sql));
        }
        if let Some(limit) = self.limit {
            sql.push(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = self.offset {
            sql.push(&format!(" OFFSET {offset}"));
        }
        sql
    }

    fn use_replica(&self) -> bool {
        self.first.use_replica() && self.rest.iter().all(|(_, leg)| leg.use_replica())
    }

    /// Fetch every combined row.
    pub async fn all<'e, E: Executor<'e>>(self, executor: E) -> Result<Vec<M>> {
        exec::fetch_all(executor, &self.to_sql(), &[], self.use_replica()).await
    }

    /// Fetch every combined row as a [`ModelCollection`].
    pub async fn get<'e, E: Executor<'e>>(self, executor: E) -> Result<ModelCollection<M>> {
        Ok(self.all(executor).await?.into())
    }

    /// Fetch the first combined row, or `None`.
    pub async fn first<'e, E: Executor<'e>>(self, executor: E) -> Result<Option<M>> {
        let replica = self.use_replica();
        exec::fetch_optional(executor, &self.limit(1).to_sql(), &[], replica).await
    }
}

impl<M> Clone for Union<M> {
    fn clone(&self) -> Self {
        Self {
            first: self.first.clone(),
            rest: self.rest.clone(),
            order: self.order.clone(),
            limit: self.limit,
            offset: self.offset,
        }
    }
}

impl<M: Model> std::fmt::Debug for Union<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Union").field(&self.to_sql()).finish()
    }
}
