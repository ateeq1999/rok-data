//! A `Select` whose results are cached.
use std::fmt;
use std::time::Duration;

use crate::exec;
use crate::sql::Sql;
use crate::{Error, Executor, Model, Page, Result};

use super::Select;

/// A [`Select`] whose results are cached, created with [`Select::memoize`].
///
/// Results are cloned out of the cache, so the model must be `Clone`.
pub struct Memoized<M> {
    pub(crate) select: Select<M>,
    pub(crate) ttl: Duration,
}

impl<M: Model + Clone> Memoized<M> {
    async fn cached<'e, T, E, F, Fut>(&self, executor: E, op: &str, sql: &Sql, run: F) -> Result<T>
    where
        T: Clone + Send + Sync + 'static,
        E: Executor<'e>,
        F: FnOnce(E) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let cache = executor.__context().and_then(|c| c.cache.clone());
        let Some(cache) = cache else {
            return run(executor).await;
        };
        let key = format!(
            "{op}\u{1f}{}\u{1f}{}\u{1f}{:?}",
            std::any::type_name::<M>(),
            sql.as_str(),
            sql.params()
        );
        if let Some(hit) = cache.get::<T>(&key) {
            tracing::debug!(target: "rok_data::cache", table = M::TABLE, op, "cache hit");
            return Ok(hit);
        }
        let stamp = cache.stamp(&self.select.tables());
        let value = run(executor).await?;
        cache.put(key, stamp, self.ttl, value.clone());
        Ok(value)
    }

    /// Fetch every matching row, from the cache when possible.
    pub async fn all<'e, E: Executor<'e>>(self, executor: E) -> Result<Vec<M>> {
        let sql = self.select.to_sql();
        let replica = self.select.use_replica();
        self.cached(executor, "all", &sql, |e| {
            exec::fetch_all(e, &sql, &[], replica)
        })
        .await
    }

    /// Fetch the first matching row, from the cache when possible.
    pub async fn first<'e, E: Executor<'e>>(self, executor: E) -> Result<Option<M>> {
        let sql = self.select.clone().limit(1).to_sql();
        self.cached(executor, "first", &sql, |e| {
            exec::fetch_optional(e, &sql, &[], self.select.use_replica())
        })
        .await
    }

    /// Fetch the first matching row, failing with [`Error::NotFound`].
    pub async fn one<'e, E: Executor<'e>>(self, executor: E) -> Result<M> {
        self.first(executor)
            .await?
            .ok_or_else(|| Error::not_found::<M>(None))
    }

    /// Count the matching rows, from the cache when possible.
    pub async fn count<'e, E: Executor<'e>>(self, executor: E) -> Result<i64> {
        let sql = self.select.count_sql();
        let replica = self.select.use_replica();
        self.cached(executor, "count", &sql, |e| {
            exec::fetch_scalar(e, &sql, replica)
        })
        .await
    }

    /// `true` if at least one row matches, from the cache when possible.
    pub async fn exists<'e, E: Executor<'e>>(self, executor: E) -> Result<bool> {
        let sql = self.select.exists_sql();
        let replica = self.select.use_replica();
        self.cached(executor, "exists", &sql, |e| {
            exec::fetch_scalar(e, &sql, replica)
        })
        .await
    }

    /// Fetch one page, from the cache when possible.
    pub async fn paginate<'e, E: Executor<'e>>(
        self,
        executor: E,
        page: u64,
        per_page: u64,
    ) -> Result<Page<M>> {
        let sql = self.select.paginate_sql(page.max(1), per_page);
        let select = self.select.clone();
        self.cached(executor, "paginate", &sql, |e| {
            select.paginate(e, page, per_page)
        })
        .await
    }
}

impl<M> Clone for Memoized<M> {
    fn clone(&self) -> Self {
        Self {
            select: self.select.clone(),
            ttl: self.ttl,
        }
    }
}

impl<M: Model> fmt::Debug for Memoized<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Memoized")
            .field("sql", &self.select.to_sql())
            .field("ttl", &self.ttl)
            .finish()
    }
}
