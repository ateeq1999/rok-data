//! A query with relations requested by `.with()`.
use std::fmt;
use std::ops::{Deref, DerefMut};

use sqlx::postgres::PgRow;

use crate::exec::{self, Journey};
use crate::relation::nested_identity;
use crate::relation::{IntoInclude, Loaded, PackedInclude, Relation, RelationExt};
use crate::{Expr, Model, Order, Page, Result};

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

    /// Adds how many records each row has through `relation`
    /// ([`Select::with_count`]).
    pub fn with_count<R: Relation<Owner = M>>(self, relation: R) -> Self {
        self.with(relation.count())
    }

    /// Adds whether each row has any record through `relation`
    /// ([`Select::with_exists`]).
    pub fn with_exists<R: Relation<Owner = M>>(self, relation: R) -> Self {
        self.with(relation.exists())
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
        load_includes(journey, &mut view, self.includes, replica).await?;
        Ok(view)
    }

    /// One page of the main rows with every requested relation loaded beside
    /// them: [`Select::paginate`] for the rows (a page and its total in one
    /// round trip), then one batched query per include over just that page.
    /// `page` is 1-based.
    ///
    /// ```ignore
    /// let page = Post::query().with(Post::AUTHOR).paginate(&db, 1, 20).await?;
    /// for post in page.rows() {
    ///     let author = page.try_get(Post::AUTHOR, post)?;
    /// }
    /// ```
    pub async fn paginate<'c, J>(
        self,
        session: J,
        page: u64,
        per_page: u64,
    ) -> Result<LoadedPage<M>>
    where
        J: Into<Journey<'c>>,
    {
        let mut journey = session.into();
        let replica = self.select.use_replica();
        let select = self.select;
        let rows: Page<M> = on_journey!(journey, |e| select.paginate(e, page, per_page).await)?;
        let Page {
            items,
            total,
            page,
            per_page,
        } = rows;
        let mut view = Loaded::new(items);
        load_includes(journey, &mut view, self.includes, replica).await?;
        Ok(LoadedPage {
            view,
            total,
            page,
            per_page,
        })
    }
}

/// One page of rows with their includes loaded, returned by
/// [`WithSelect::paginate`]: the page's numbers beside the [`Loaded`] view,
/// which it `Deref`s to — `page.rows()`, `page.try_get(Post::AUTHOR, &post)`.
#[derive(Debug)]
pub struct LoadedPage<M: Model> {
    /// The rows on this page and their loaded relations.
    pub view: Loaded<M>,
    /// Total number of matching rows across all pages.
    pub total: u64,
    /// Current page number (1-based).
    pub page: u64,
    /// Maximum number of rows per page.
    pub per_page: u64,
}

impl<M: Model> LoadedPage<M> {
    /// The page's numbers as a [`Page`] of its rows, for every page helper
    /// (`last_page`, `first_item`, `has_next`, …); the loaded relations are
    /// dropped.
    pub fn into_page(self) -> Page<M> {
        Page::new(self.view.into_rows(), self.total, self.page, self.per_page)
    }

    /// The number of the last page (at least 1).
    pub fn last_page(&self) -> u64 {
        Page::<()>::new(Vec::new(), self.total, self.page, self.per_page).last_page()
    }

    /// `true` if a later page exists.
    pub fn has_next(&self) -> bool {
        self.page < self.last_page()
    }
}

impl<M: Model> Deref for LoadedPage<M> {
    type Target = Loaded<M>;

    fn deref(&self) -> &Loaded<M> {
        &self.view
    }
}

impl<M: Model> DerefMut for LoadedPage<M> {
    fn deref_mut(&mut self) -> &mut Loaded<M> {
        &mut self.view
    }
}

/// The include loop shared by the with-loading entry points: one batched
/// query per include, in order, each parked on `view` afterwards. The
/// main rows must already be in the view — [`WithSelect::all`] fetched
/// them, [`ModelCollection::load`](crate::ModelCollection::load) and
/// [`Loaded::load_missing`](crate::relation::Loaded::load_missing) carry
/// theirs. The session arrives owned (a [`Journey`]); every leg of the
/// loop borrows it.
///
/// Repeated identities overwrite (the relation included twice, last
/// shaping wins — [`load_missing`](crate::relation::Loaded::load_missing)
/// filters its own repeats out first).
pub(crate) async fn load_includes<M: Model>(
    mut journey: Journey<'_>,
    view: &mut Loaded<M>,
    includes: Vec<PackedInclude<M>>,
    replica: bool,
) -> Result<()> {
    for include in includes {
        // The SQL is rendered here, once the owners' key values are known.
        let include_sql = (include.children_sql)(view.rows())?;
        let children: Vec<PgRow> =
            on_journey!(journey, |e| exec::fetch_rows(e, &include_sql, &[], replica)
                .await)?;
        let state = (include.assemble)(children)?;
        if let Some(nested) = include.nested {
            let view_of_targets = nested(journey.reborrow(), &state, replica).await?;
            view.park(nested_identity(include.identity), view_of_targets);
        }
        view.park(include.identity, state);
    }
    Ok(())
}

/// Runs `sql` on `journey` and returns its rows, the session kept for
/// the next leg.
pub(crate) async fn fetch_rows_on(
    journey: &mut Journey<'_>,
    sql: &crate::sql::Sql,
) -> Result<Vec<PgRow>> {
    on_journey!(*journey, |e| exec::fetch_rows(e, sql, &[], false).await)
}
