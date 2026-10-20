//! Building a `Db` from pools, a cache, and read replicas.

use std::sync::Arc;
use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

use crate::context::Context;
use crate::{Error, QueryCache, Result};

/// A pooled PostgreSQL connection handle — the entry point of rok-data.
///
/// `Db` is cheap to clone (it wraps an `Arc`'d pool) and can be passed
/// wherever an [`Executor`](crate::Executor) is expected:
///
/// ```ignore
/// let db = Db::connect("postgres://localhost/app").await?;
/// let user = User::find(&db, 1).await?;
/// ```
use super::Db;

/// Builder for a [`Db`] connection pool, created with [`Db::builder`].
#[derive(Debug, Clone, Default)]
pub struct DbBuilder {
    options: PgPoolOptions,
    ctx: Context,
    replica_urls: Vec<String>,
    shared_invalidation: bool,
}

impl DbBuilder {
    /// Enable the [query cache](QueryCache) used by
    /// [`Select::memoize`](crate::Select::memoize), holding up to
    /// `capacity` results.
    pub fn query_cache(mut self, capacity: usize) -> Self {
        self.ctx.cache = Some(QueryCache::new(capacity));
        self
    }

    /// Use an existing [`QueryCache`], e.g. one shared by several pools.
    pub fn with_query_cache(mut self, cache: QueryCache) -> Self {
        self.ctx.cache = Some(cache);
        self
    }

    /// Add a read replica. Reads made through `&Db` by the query builder
    /// (`all`, `first`, `count`, `paginate`, relations, …) are spread across
    /// replicas round-robin; writes, row locks (`for_update`), transactions
    /// and raw SQL stay on the primary. A read that fails because a replica
    /// is unreachable is retried on the primary.
    ///
    /// Replicas lag behind the primary: use
    /// [`Select::on_primary`](crate::Select::on_primary) or
    /// [`Db::primary`] where a read must see the latest write.
    pub fn read_replica(mut self, url: impl Into<String>) -> Self {
        self.replica_urls.push(url.into());
        self
    }

    /// Add an existing pool as a read replica (see [`read_replica`](Self::read_replica)).
    pub fn replica_pool(mut self, pool: PgPool) -> Self {
        self.ctx.replicas.push(pool);
        self
    }

    /// Keep the query caches of every process using this database coherent:
    /// invalidations are broadcast with PostgreSQL `NOTIFY` and applied by
    /// all instances that enabled this (needs [`query_cache`](Self::query_cache)
    /// and a Tokio runtime). A background task holds one extra connection.
    ///
    /// Invalidation is asynchronous: other instances drop stale entries a few
    /// milliseconds after the writing transaction's statement. After a lost
    /// connection the local cache is cleared, since messages may have been
    /// missed.
    pub fn shared_cache_invalidation(mut self) -> Self {
        self.shared_invalidation = true;
        self
    }

    /// Log queries slower than `threshold` at `WARN` level on the
    /// `rok_data::slow_query` tracing target (default 1s, `Duration::ZERO`
    /// disables it).
    pub fn slow_query_threshold(mut self, threshold: Duration) -> Self {
        self.ctx.slow_query = threshold;
        self
    }

    fn build(self, pool: PgPool) -> Db {
        if self.shared_invalidation {
            match &self.ctx.cache {
                Some(cache) => crate::cache_sync::start(pool.clone(), cache),
                None => {
                    tracing::warn!(target: "rok_data::cache", "shared_cache_invalidation() without query_cache(); ignored")
                }
            }
        }
        Db {
            pool,
            ctx: Arc::new(self.ctx),
        }
    }

    /// Maximum number of pooled connections (default 10).
    pub fn max_connections(mut self, n: u32) -> Self {
        self.options = self.options.max_connections(n);
        self
    }

    /// Minimum number of idle connections kept open (default 0).
    pub fn min_connections(mut self, n: u32) -> Self {
        self.options = self.options.min_connections(n);
        self
    }

    /// How long to wait for a free connection (default 30s).
    pub fn acquire_timeout(mut self, timeout: Duration) -> Self {
        self.options = self.options.acquire_timeout(timeout);
        self
    }

    /// Close connections idle for longer than `timeout` (default 10min).
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.options = self.options.idle_timeout(timeout);
        self
    }

    /// Recycle connections older than `lifetime` (default 30min).
    pub fn max_lifetime(mut self, lifetime: Duration) -> Self {
        self.options = self.options.max_lifetime(lifetime);
        self
    }

    /// Connect to `url` and return the pool.
    pub async fn connect(self, url: &str) -> Result<Db> {
        let pool = self.options.clone().connect(url).await?;
        self.finish(pool).await
    }

    /// Connect the configured replica URLs, then build the handle.
    async fn finish(mut self, pool: PgPool) -> Result<Db> {
        for url in std::mem::take(&mut self.replica_urls) {
            let replica = self.options.clone().connect(&url).await?;
            self.ctx.replicas.push(replica);
        }
        Ok(self.build(pool))
    }

    /// Connect using the `DATABASE_URL` environment variable.
    pub async fn connect_env(self) -> Result<Db> {
        let url = std::env::var("DATABASE_URL").map_err(|_| {
            Error::Config("the `DATABASE_URL` environment variable is not set".into())
        })?;
        self.connect(&url).await
    }

    /// Wrap an existing sqlx pool, applying this builder's rok-data settings
    /// (pool options and replica URLs are ignored; use
    /// [`replica_pool`](Self::replica_pool) for replicas).
    pub fn build_with_pool(self, pool: PgPool) -> Db {
        self.build(pool)
    }

    /// Connect with fully custom connect options.
    pub async fn connect_with(self, options: PgConnectOptions) -> Result<Db> {
        let pool = self.options.clone().connect_with(options).await?;
        self.finish(pool).await
    }

    /// Create the pool without opening any connection until first use.
    pub fn connect_lazy(self, url: &str) -> Result<Db> {
        let pool = self.options.clone().connect_lazy(url)?;
        let mut builder = self;
        for url in std::mem::take(&mut builder.replica_urls) {
            let replica = builder.options.clone().connect_lazy(&url)?;
            builder.ctx.replicas.push(replica);
        }
        Ok(builder.build(pool))
    }
}
