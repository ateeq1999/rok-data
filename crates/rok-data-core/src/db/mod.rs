//! The database handle: a pool, its replicas, its cache, and its transactions.

use std::fmt;

use std::sync::{Arc, Mutex};

use futures_core::future::BoxFuture;
use sqlx::postgres::PgPool;

use crate::context::Context;
use crate::{Error, QueryCache, Result};

mod builder;
mod options;
mod retry;
mod stats;
mod tx;

pub use builder::DbBuilder;
pub use options::{Isolation, TxOptions};
pub use retry::Retryable;
pub use stats::PoolStats;
pub use tx::Tx;

/// A pooled PostgreSQL connection handle — the entry point of rok-data.
///
/// `Db` is cheap to clone (it wraps an `Arc`'d pool) and can be passed
/// wherever an [`Executor`](crate::Executor) is expected:
///
/// ```ignore
/// let db = Db::connect("postgres://localhost/app").await?;
/// let user = User::find(&db, 1).await?;
/// ```
#[derive(Clone)]
pub struct Db {
    pool: PgPool,
    ctx: Arc<Context>,
}

impl Db {
    /// Connect with default pool settings.
    pub async fn connect(url: &str) -> Result<Self> {
        Self::builder().connect(url).await
    }

    /// Connect using the `DATABASE_URL` environment variable.
    pub async fn connect_env() -> Result<Self> {
        Self::builder().connect_env().await
    }

    /// Configure the connection pool before connecting.
    pub fn builder() -> DbBuilder {
        DbBuilder::default()
    }

    /// Wrap an existing sqlx pool.
    pub fn from_pool(pool: PgPool) -> Self {
        Self {
            pool,
            ctx: Arc::default(),
        }
    }

    /// A handle to the same pool and settings that never uses read replicas,
    /// for reads that must see the latest writes.
    pub fn primary(&self) -> Db {
        let mut ctx = (*self.ctx).clone();
        ctx.replicas.clear();
        Db {
            pool: self.pool.clone(),
            ctx: Arc::new(ctx),
        }
    }

    /// Number of configured read replicas.
    pub fn replica_count(&self) -> usize {
        self.ctx.replicas.len()
    }

    /// A snapshot of the connection pool's state.
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            size: self.pool.size(),
            idle: self.pool.num_idle(),
            max_connections: self.pool.options().get_max_connections(),
        }
    }

    /// Report [`stats`](Self::stats) as `rok_data_pool_connections` gauges
    /// (feature `metrics`). Call it periodically, e.g. from a background
    /// task or before each metrics scrape.
    #[cfg(feature = "metrics")]
    pub fn record_pool_metrics(&self) {
        crate::metrics::pool(self.stats());
    }

    /// The query cache, if enabled with [`DbBuilder::query_cache`].
    pub fn cache(&self) -> Option<&QueryCache> {
        self.ctx.cache.as_ref()
    }

    /// The underlying sqlx pool, for anything rok-data doesn't cover.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Consume the handle and return the underlying sqlx pool.
    pub fn into_pool(self) -> PgPool {
        self.pool
    }

    /// Start a transaction. It is rolled back on drop unless
    /// [`Tx::commit`] is called.
    pub async fn begin(&self) -> Result<Tx> {
        #[cfg_attr(not(feature = "json"), allow(unused_mut))]
        let mut tx = Tx {
            inner: self.pool.begin().await?,
            ctx: Some(self.ctx.clone()),
            touched: Mutex::default(),
        };
        #[cfg(feature = "json")]
        if let Some(actor) = crate::audit::current_actor() {
            tx.set_actor(&actor).await?;
        }
        Ok(tx)
    }

    /// Run `f` inside a transaction, committing if it returns `Ok` and
    /// rolling back if it returns `Err`.
    ///
    /// ```ignore
    /// let user = db.transaction(|tx| Box::pin(async move {
    ///     let user = new_user.insert(&mut *tx).await?;
    ///     Profile::create().set(Profile::USER_ID, user.id).exec(&mut *tx).await?;
    ///     Ok(user)
    /// })).await?;
    /// ```
    pub async fn transaction<T, E, F>(&self, f: F) -> std::result::Result<T, E>
    where
        F: for<'t> FnOnce(&'t mut Tx) -> BoxFuture<'t, std::result::Result<T, E>>,
        E: From<Error>,
    {
        let mut tx = self.begin().await?;
        match f(&mut tx).await {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(err) => {
                tx.rollback().await?;
                Err(err)
            }
        }
    }

    /// Run `f` in a transaction configured by `options`, retrying the whole
    /// closure when PostgreSQL aborts it with a serialization failure or a
    /// deadlock (SQLSTATE `40001` / `40P01`), up to `options.retries` times.
    ///
    /// The closure may run more than once, so it must be `Fn` and should not
    /// have side effects outside the database.
    ///
    /// ```ignore
    /// let opts = TxOptions::new().isolation(Isolation::Serializable).retries(5);
    /// db.transaction_with(opts, |tx| Box::pin(async move {
    ///     let mut from = Account::find_or_fail(&mut *tx, 1).await?;
    ///     let mut to = Account::find_or_fail(&mut *tx, 2).await?;
    ///     from.balance -= 10;
    ///     to.balance += 10;
    ///     from.save(&mut *tx).await?;
    ///     to.save(&mut *tx).await?;
    ///     Ok::<_, rok_data::Error>(())
    /// })).await?;
    /// ```
    pub async fn transaction_with<T, E, F>(
        &self,
        options: TxOptions,
        f: F,
    ) -> std::result::Result<T, E>
    where
        F: for<'t> Fn(&'t mut Tx) -> BoxFuture<'t, std::result::Result<T, E>>,
        E: From<Error> + Retryable,
    {
        let mut attempt = 0;
        loop {
            let result = self.try_transaction(&options, &f).await;
            match result {
                Err(err) if attempt < options.retries && err.is_retryable() => {
                    attempt += 1;
                    let delay = options.backoff(attempt);
                    tracing::debug!(target: "rok_data::query", attempt, delay_ms = delay.as_millis() as u64, "retrying transaction");
                    tokio::time::sleep(delay).await;
                }
                other => return other,
            }
        }
    }

    async fn try_transaction<T, E, F>(
        &self,
        options: &TxOptions,
        f: &F,
    ) -> std::result::Result<T, E>
    where
        F: for<'t> Fn(&'t mut Tx) -> BoxFuture<'t, std::result::Result<T, E>>,
        E: From<Error>,
    {
        let mut tx = self.begin().await?;
        if let Some(sql) = options.set_transaction_sql() {
            sqlx::query(sqlx::AssertSqlSafe(sql))
                .execute(&mut *tx.inner)
                .await
                .map_err(Error::from)?;
        }
        match f(&mut tx).await {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(err) => {
                // The transaction may already be aborted; a failed rollback
                // must not hide the original error.
                let _ = tx.rollback().await;
                Err(err)
            }
        }
    }

    /// Execute one or more raw SQL statements without parameters (e.g. a
    /// schema script) and return the total number of affected rows.
    pub async fn execute(&self, sql: &str) -> Result<u64> {
        Ok(sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&self.pool)
            .await?
            .rows_affected())
    }

    /// Check that the database is reachable.
    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    /// Run every pending migration found in the `dir` directory
    /// (sqlx migration format).
    #[cfg(feature = "migrate")]
    pub async fn migrate(&self, dir: impl AsRef<std::path::Path>) -> Result<()> {
        let migrator = sqlx::migrate::Migrator::new(dir.as_ref()).await?;
        migrator.run(&self.pool).await?;
        Ok(())
    }

    /// Close every connection in the pool and its replicas.
    pub async fn close(&self) {
        self.pool.close().await;
        for replica in &self.ctx.replicas {
            replica.close().await;
        }
    }
}

impl From<PgPool> for Db {
    fn from(pool: PgPool) -> Self {
        Self::from_pool(pool)
    }
}

impl fmt::Debug for Db {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Db")
            .field("size", &self.pool.size())
            .field("idle", &self.pool.num_idle())
            .field("cache", &self.ctx.cache)
            .finish()
    }
}
