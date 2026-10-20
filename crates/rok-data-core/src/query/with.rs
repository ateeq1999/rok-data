//! A query with relations requested by `.with()`.
use std::fmt;

use sqlx::postgres::PgRow;

use crate::exec::{self, Journey};
use crate::relation::{IntoInclude, Loaded, PackedInclude};
use crate::{Expr, Model, Order, Result};

use super::Select;

/// A query with relations requested by [`.with()`](Select::with): the main
/// rows plus one batched load per include, fetched on one shared
/// connection ([`Journey`](crate::exec::Journey)). Included rows ride out
/// in the [`Loaded`](crate::relation::Loaded) map of `all`.
///
/// Chaining keeps the includes: `filter`, `order_by`, `limit` and `offset`
/// act on the main rows, and `.with()` adds relations one after another.
pub struct WithSelect<M: Model> {
    pub(crate) select: Select<M>,
    pub(crate) includes: Vec<PackedInclude<M>>,
}

impl<M: Model> fmt::Debug for WithSelect<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WithSelect")
            .field("select", &self.select.to_sql())
            .field("includes", &self.includes.len())
            .finish()
    }
}

impl<M: Model> WithSelect<M> {
    /// Restricts the main rows; the includes keep their own shaping.
    pub fn filter(mut self, expr: Expr<M>) -> Self {
        self.select = self.select.filter(expr);
        self
    }

    /// Orders the main rows.
    pub fn order_by(mut self, order: impl Into<Order<M>>) -> Self {
        self.select = self.select.order_by(order);
        self
    }

    /// Caps the main rows.
    pub fn limit(mut self, limit: u64) -> Self {
        self.select = self.select.limit(limit);
        self
    }

    /// Skips that many matching main rows.
    pub fn offset(mut self, offset: u64) -> Self {
        self.select = self.select.offset(offset);
        self
    }

    /// Adds one more relation to load beside the rows.
    pub fn with<I: IntoInclude<M>>(mut self, include: I) -> Self {
        self.includes.push(include.into_packed());
        self
    }

    /// Fetches the rows and every requested relation: the main query,
    /// then one batched query per include, in the order the includes were
    /// added.
    pub async fn all<'c, J>(self, session: J) -> Result<Loaded<M>>
    where
        J: Into<Journey<'c>>,
    {
        let mut journey = session.into();
        let replica = self.select.use_replica();
        let sql = self.select.to_sql();
        let rows: Vec<M> = on_journey!(journey, |e| exec::fetch_all(e, &sql, &[], replica).await)?;
        let mut view = Loaded::new(rows);
        for include in self.includes {
            // The SQL is rendered here, once the owners' key values are known.
            let include_sql = (include.children_sql)(view.rows())?;
            let children: Vec<PgRow> =
                on_journey!(journey, |e| exec::fetch_rows(e, &include_sql, &[], replica)
                    .await)?;
            view.park(include.identity, (include.assemble)(children)?);
        }
        Ok(view)
    }
}
