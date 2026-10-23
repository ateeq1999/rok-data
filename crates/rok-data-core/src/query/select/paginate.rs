//! Offset pagination: [`Select::paginate`] (a page with its total),
//! [`Select::page`] (the same at the model's page size),
//! [`Select::simple_paginate`] (a page and whether more follow, no
//! `COUNT(*)`), and [`Select::for_page`] (the window as chain sugar).

use sqlx::Row;

use crate::exec;
use crate::model::Model;
use crate::{Error, Executor, Page, Result, SimplePage};

use super::Select;

impl<M: Model> Select<M> {
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

    /// One page at the model's page size — [`paginate`](Self::paginate)
    /// with [`Model::PER_PAGE`] rows (15, or `#[rok(per_page = N)]`):
    /// Laravel's `paginate()` without an argument. `page` is 1-based.
    pub async fn page<'e, E: Executor<'e>>(self, executor: E, page: u64) -> Result<Page<M>> {
        self.paginate(executor, page, M::PER_PAGE).await
    }

    /// One page and whether another follows, in a single query and without
    /// a `COUNT(*)` — Laravel's `simplePaginate`, for a "next" link where
    /// the total does not matter or costs too much. Reads `per_page + 1`
    /// rows and keeps `per_page`. `page` is 1-based.
    pub async fn simple_paginate<'e, E: Executor<'e>>(
        self,
        executor: E,
        page: u64,
        per_page: u64,
    ) -> Result<SimplePage<M>> {
        if per_page == 0 {
            return Err(Error::InvalidQuery(
                "`per_page` must be greater than 0".into(),
            ));
        }
        let page = page.max(1);
        let mut items = self
            .for_page(page, per_page)
            .limit(per_page.saturating_add(1))
            .all(executor)
            .await?;
        let has_more = items.len() as u64 > per_page;
        items.truncate(per_page as usize);
        Ok(SimplePage::new(items, page, per_page, has_more))
    }

    /// Narrows the chain to page `page` of `per_page` rows (`LIMIT` and
    /// `OFFSET`), 1-based — Laravel's `forPage`, for when the total is not
    /// wanted at all.
    pub fn for_page(self, page: u64, per_page: u64) -> Self {
        let offset = (page.max(1) - 1).saturating_mul(per_page);
        self.limit(per_page).offset(offset)
    }
}
