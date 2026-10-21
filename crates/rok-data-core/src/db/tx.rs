//! The transaction handle.

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
use sqlx::postgres::{PgConnection, PgPool, PgQueryResult, PgRow, PgStatement, PgTypeInfo};
use sqlx::{Describe, Either, Execute, Postgres};

use crate::Result;
use crate::context::Context;

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

/// Pass `&mut *tx` wherever an [`Executor`](crate::Executor) is expected.
/// A transaction that is dropped without [`commit`](Tx::commit) is rolled back.
pub struct Tx {
    pub(crate) inner: sqlx::Transaction<'static, Postgres>,
    pub(crate) ctx: Option<Arc<Context>>,
    pub(crate) touched: Mutex<Vec<&'static str>>,
}

impl Tx {
    /// Commit the transaction.
    pub async fn commit(self) -> Result<()> {
        self.inner.commit().await?;
        // Results cached by other connections while this transaction was
        // open still reflect the old data.
        if let Some(cache) = self.ctx.as_ref().and_then(|c| c.cache.as_ref()) {
            let touched = self.touched.lock().unwrap_or_else(|e| e.into_inner());
            for table in touched.iter() {
                cache.invalidate(table);
            }
        }
        Ok(())
    }

    /// Roll the transaction back.
    pub async fn rollback(self) -> Result<()> {
        Ok(self.inner.rollback().await?)
    }

    /// Record `actor` as the author of this transaction's changes in the
    /// audit log (`SET LOCAL rok.actor`).
    #[cfg_attr(
        feature = "json",
        doc = "See the [`audit`](crate::audit) module for the audit trail."
    )]
    pub async fn set_actor(&mut self, actor: &str) -> Result<()> {
        sqlx::query("SELECT set_config('rok.actor', $1, true)")
            .bind(actor)
            .execute(&mut *self.inner)
            .await?;
        Ok(())
    }

    /// The underlying sqlx transaction.
    pub fn inner(&mut self) -> &mut sqlx::Transaction<'static, Postgres> {
        &mut self.inner
    }

    /// Record a write to `table` made outside the executor path (e.g. COPY).
    pub(crate) fn note_write(&self, table: &'static str) {
        let mut touched = self.touched.lock().unwrap_or_else(|e| e.into_inner());
        if !touched.contains(&table) {
            touched.push(table);
        }
        if let Some(cache) = self.ctx.as_ref().and_then(|c| c.cache.as_ref()) {
            cache.invalidate(table);
        }
    }
}

impl From<sqlx::Transaction<'static, Postgres>> for Tx {
    fn from(inner: sqlx::Transaction<'static, Postgres>) -> Self {
        Self {
            inner,
            ctx: None,
            touched: Mutex::default(),
        }
    }
}

impl Deref for Tx {
    type Target = PgConnection;

    fn deref(&self) -> &PgConnection {
        &self.inner
    }
}

impl DerefMut for Tx {
    fn deref_mut(&mut self) -> &mut PgConnection {
        &mut self.inner
    }
}

impl fmt::Debug for Tx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Tx")
    }
}

impl<'c> crate::Executor<'c> for &'c Db {
    fn __context(&self) -> Option<Arc<Context>> {
        Some(self.ctx.clone())
    }

    fn __replica(&self) -> Option<PgPool> {
        self.ctx.replica()
    }
}

impl<'c> crate::Executor<'c> for &'c mut Tx {
    fn __context(&self) -> Option<Arc<Context>> {
        self.ctx.clone()
    }

    fn __touch(&self, table: &'static str) {
        let mut touched = self.touched.lock().unwrap_or_else(|e| e.into_inner());
        if !touched.contains(&table) {
            touched.push(table);
        }
    }
}

/// Implement sqlx's `Executor` by delegating to an inner executor.
macro_rules! delegate_executor {
    ($ty:ty, |$this:ident| $inner:expr) => {
        impl<'c> sqlx::Executor<'c> for $ty {
            type Database = Postgres;

            fn fetch_many<'e, 'q: 'e, E>(
                self,
                query: E,
            ) -> BoxStream<'e, std::result::Result<Either<PgQueryResult, PgRow>, sqlx::Error>>
            where
                'c: 'e,
                E: 'q + Execute<'q, Postgres>,
            {
                let $this = self;
                $inner.fetch_many(query)
            }

            fn fetch_optional<'e, 'q: 'e, E>(
                self,
                query: E,
            ) -> BoxFuture<'e, std::result::Result<Option<PgRow>, sqlx::Error>>
            where
                'c: 'e,
                E: 'q + Execute<'q, Postgres>,
            {
                let $this = self;
                $inner.fetch_optional(query)
            }

            fn prepare_with<'e>(
                self,
                sql: sqlx::SqlStr,
                parameters: &'e [PgTypeInfo],
            ) -> BoxFuture<'e, std::result::Result<PgStatement, sqlx::Error>>
            where
                'c: 'e,
            {
                let $this = self;
                $inner.prepare_with(sql, parameters)
            }

            fn describe<'e>(
                self,
                sql: sqlx::SqlStr,
            ) -> BoxFuture<'e, std::result::Result<Describe<Postgres>, sqlx::Error>>
            where
                'c: 'e,
            {
                let $this = self;
                $inner.describe(sql)
            }
        }
    };
}

delegate_executor!(&'c Db, |this| &this.pool);
delegate_executor!(&'c mut Tx, |this| &mut *this.inner);
