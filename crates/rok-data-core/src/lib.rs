//! Core runtime of [rok-data](https://docs.rs/rok-data).
//!
//! Most users should depend on the `rok-data` crate, which re-exports
//! everything here together with the `#[derive(Model)]` macro.

#![allow(async_fn_in_trait)]

#[cfg(feature = "json")]
pub mod audit;
mod cache;
mod cache_sync;
mod collection;
mod context;
mod copy;
mod cursor;
mod db;
mod error;
mod exec;
mod expr;
pub mod introspect;
pub mod join;
mod key;
pub mod metrics;
pub mod migration;
pub mod migration_file;
mod model;
pub mod notify;
mod page;
mod query;
mod raw;
pub mod relation;
mod simple_page;
mod sql;
pub mod tenant;
#[cfg(feature = "testing")]
pub mod testing;
mod tracked;
pub mod validate;
mod value;
pub mod web;

pub use cache::{CacheStats, QueryCache};
pub use collection::ModelCollection;
pub use context::Executor;
pub use copy::CopyTarget;
pub use cursor::{Cursor, CursorPage};
pub use db::{Db, DbBuilder, Isolation, PoolStats, Retryable, Tx, TxOptions};
pub use error::{Error, Result};
pub use exec::Journey;
pub use expr::{Column, Direction, Expr, IntoProjections, Order, Projection};
pub use join::{InScope, IntoJoin, JoinOn, Joined};
pub use key::IntoKey;
pub use model::{Hooks, Model, ModelWrite};
pub use notify::{Change, ChangeOp, ChangeStream, Listener, Notification};
pub use page::Page;
pub use query::{Insert, InsertMany, LoadedPage, Memoized, Projected, Select, Update, WithSelect};
pub use raw::{Raw, raw};
pub use relation::{BelongsTo, HasMany, HasOne, Include, Loaded, Relation};
pub use simple_page::SimplePage;
pub use sql::Sql;
pub use tracked::Tracked;
pub use validate::ValidationErrors;
pub use value::{CustomType, CustomValue, Value};

/// A boxed `Send` future, as returned by [`Db::transaction`] closures.
pub use futures_core::future::BoxFuture;
/// A boxed `Send` stream, as returned by [`Select::stream`].
pub use futures_core::stream::BoxStream;
/// Extension traits for consuming streams (`try_next`, `try_collect`, …).
pub use futures_util::{StreamExt, TryStreamExt};

/// The sqlx crate rok-data is built on, re-exported for advanced use.
pub use sqlx;

#[doc(hidden)]
pub mod __private {
    pub use crate::context::Context;
    pub use crate::model::__insert_sql;
    pub use crate::query::{__cursor_sql, __paginate_sql};
    pub use sqlx::encode::IsNull;
    pub use sqlx::error::BoxDynError;
    pub use sqlx::postgres::{PgArgumentBuffer, PgRow, PgTypeInfo, PgValueRef};
    pub use sqlx::{Decode, Encode, Error as SqlxError, FromRow, Postgres, Row, Type};
}
