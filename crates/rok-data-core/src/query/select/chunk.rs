//! Walking a large table: [`Select::chunk`] (one streamed query, in the
//! query's order), [`Select::chunk_by_id`] (one keyset query per window,
//! by primary key — safe when the callback changes the rows it was given),
//! the lazy streams built on the same windows ([`Select::lazy`],
//! [`Select::lazy_by_id`], [`Select::lazy_by_id_desc`]), and the
//! row-at-a-time walks ([`Select::each`], [`Select::each_by_id`]).

use std::future::Future;
use std::ops::ControlFlow;

use futures_core::stream::BoxStream;
use futures_util::stream;

use crate::cursor::Cursor;
use crate::exec::Journey;
use crate::model::Model;
use crate::{Column, Error, Result, TryStreamExt};

use super::Select;

/// The rows a row-at-a-time walk ([`Select::each`]) reads per query.
const EACH_WINDOW: usize = 1000;

/// Whether a walk goes on after a callback — what a [`Select::chunk`] or
/// [`Select::each`] callback returns. `()` always goes on; `false` (or
/// [`ControlFlow::Break`]) stops the walk there:
///
/// ```ignore
/// User::query().chunk_by_id(&db, 100, |rows| async move {
///     Ok(rows.len() == 100) // stop after the first short window
/// }).await?;
/// ```
pub trait Walk {
    /// `true` to go on to the next window or row.
    fn goes_on(self) -> bool;
}

impl Walk for () {
    fn goes_on(self) -> bool {
        true
    }
}

impl Walk for bool {
    fn goes_on(self) -> bool {
        self
    }
}

impl Walk for ControlFlow<()> {
    fn goes_on(self) -> bool {
        self.is_continue()
    }
}

impl<M: Model> Select<M> {
    /// Process the matching rows in windows of `size`, handing each window to
    /// `f`, in the query's order.
    ///
    /// Each window is its own `LIMIT`/`OFFSET` query on the session (an
    /// unordered query is ordered by its primary key), so only
    /// one window is in memory and no connection is held while `f` runs: a
    /// callback that writes back through the same pool works with a pool of
    /// one. `f` returns `Ok(())` to go on, or `Ok(false)` to stop (see
    /// [`Walk`]); the first error stops the walk and is returned.
    ///
    /// ```ignore
    /// let db = &db; // a reference the `async move` blocks copy
    /// User::query().chunk(db, 100, |rows| async move {
    ///     for user in rows {
    ///         archive(db, user).await?;
    ///     }
    ///     Ok(())
    /// }).await?;
    /// ```
    ///
    /// A callback that deletes or re-sorts the rows it is handed shifts the
    /// offsets, so later windows skip rows: use
    /// [`chunk_by_id`](Self::chunk_by_id) for that.
    pub async fn chunk<'c, S, F, Fut, W>(self, session: S, size: usize, mut f: F) -> Result<()>
    where
        S: Into<Journey<'c>>,
        F: FnMut(Vec<M>) -> Fut,
        Fut: Future<Output = Result<W>>,
        W: Walk,
    {
        let mut windows = self.offset_windows(session.into(), size)?;
        while let Some(window) = windows.try_next().await? {
            if !f(window).await?.goes_on() {
                break;
            }
        }
        Ok(())
    }

    /// Process the matching rows in windows of `size`, by primary key.
    /// Each window is its own keyset query (`WHERE
    /// key > the last window's last key ORDER BY key LIMIT size`), so no
    /// connection is held between windows and the walk stays correct when
    /// `f` updates or deletes the rows it was handed: every row that
    /// matches is visited exactly once. The query's own `ORDER BY` is
    /// replaced by the primary key (composite keys included).
    ///
    /// The session is anything a query takes — the pool, the database, a
    /// transaction — and every window's query runs on it.
    ///
    /// ```ignore
    /// let db = &db;
    /// User::filter(User::ACTIVE.eq(false)).chunk_by_id(db, 500, |rows| async move {
    ///     for user in rows {
    ///         user.delete(db).await?; // safe: the next window starts after the key
    ///     }
    ///     Ok(())
    /// }).await?;
    /// ```
    pub async fn chunk_by_id<'c, S, F, Fut, W>(
        self,
        session: S,
        size: usize,
        mut f: F,
    ) -> Result<()>
    where
        S: Into<Journey<'c>>,
        F: FnMut(Vec<M>) -> Fut,
        Fut: Future<Output = Result<W>>,
        W: Walk,
    {
        let mut windows = self.by_key(false).windows(session.into(), size)?;
        while let Some(window) = windows.try_next().await? {
            if !f(window).await?.goes_on() {
                break;
            }
        }
        Ok(())
    }

    /// The matching rows as a stream, read `size` at a time in the query's
    /// order (`LIMIT`/`OFFSET` windows). One window is in
    /// memory at a time, and no connection is held between windows; for
    /// rows that change during the walk, prefer
    /// [`lazy_by_id`](Self::lazy_by_id).
    pub fn lazy<'c, S>(self, session: S, size: usize) -> Result<BoxStream<'c, Result<M>>>
    where
        S: Into<Journey<'c>>,
        M: 'c,
    {
        Ok(flatten(self.offset_windows(session.into(), size)?))
    }

    /// The matching rows as a stream, read `size` at a time by primary key
    /// ascending: keyset windows, correct while the
    /// rows change, no connection held between windows.
    pub fn lazy_by_id<'c, S>(self, session: S, size: usize) -> Result<BoxStream<'c, Result<M>>>
    where
        S: Into<Journey<'c>>,
        M: 'c,
    {
        Ok(flatten(self.by_key(false).windows(session.into(), size)?))
    }

    /// [`lazy_by_id`](Self::lazy_by_id), by primary key descending.
    pub fn lazy_by_id_desc<'c, S>(self, session: S, size: usize) -> Result<BoxStream<'c, Result<M>>>
    where
        S: Into<Journey<'c>>,
        M: 'c,
    {
        Ok(flatten(self.by_key(true).windows(session.into(), size)?))
    }

    /// Hand every matching row to `f`, one at a time, in the query's order:
    /// [`chunk`](Self::chunk) underneath, a thousand rows
    /// a window, no connection held while `f` runs. `f` returns `Ok(())` to
    /// go on or `Ok(false)` to stop.
    pub async fn each<'c, S, F, Fut, W>(self, session: S, mut f: F) -> Result<()>
    where
        S: Into<Journey<'c>>,
        F: FnMut(M) -> Fut,
        Fut: Future<Output = Result<W>>,
        W: Walk,
    {
        let mut windows = self.offset_windows(session.into(), EACH_WINDOW)?;
        while let Some(window) = windows.try_next().await? {
            for row in window {
                if !f(row).await?.goes_on() {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// Hand every matching row to `f`, one at a time, by primary key:
    /// [`chunk_by_id`](Self::chunk_by_id)
    /// underneath, a thousand rows a window, so `f` may change the rows.
    pub async fn each_by_id<'c, S, F, Fut, W>(self, session: S, mut f: F) -> Result<()>
    where
        S: Into<Journey<'c>>,
        F: FnMut(M) -> Fut,
        Fut: Future<Output = Result<W>>,
        W: Walk,
    {
        let mut windows = self.by_key(false).windows(session.into(), EACH_WINDOW)?;
        while let Some(window) = windows.try_next().await? {
            for row in window {
                if !f(row).await?.goes_on() {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// This query ordered by its primary key alone, ascending or
    /// descending — the order the `*_by_id` walks follow.
    fn by_key(mut self, descending: bool) -> Self {
        self.order = M::PRIMARY_KEYS
            .iter()
            .map(|key| {
                let column = Column::<M>::new(key);
                if descending {
                    column.desc()
                } else {
                    column.asc()
                }
            })
            .collect();
        self
    }

    /// The `LIMIT`/`OFFSET` windows of this query, in its own order: one
    /// `for_page` query per window on `journey`, until a short window. A
    /// query without an `ORDER BY` is ordered by its primary key: offsets
    /// over an unordered query
    /// are not stable from one window's query to the next.
    fn offset_windows<'c>(
        mut self,
        journey: Journey<'c>,
        size: usize,
    ) -> Result<BoxStream<'c, Result<Vec<M>>>>
    where
        M: 'c,
    {
        if size == 0 {
            return Err(Error::InvalidQuery("`size` must be greater than 0".into()));
        }
        if self.order.is_empty() {
            self = self.by_key(false);
        }
        let size = size as u64;
        let windows = stream::try_unfold(
            (journey, self, 1_u64, false),
            move |(mut journey, select, page, done)| async move {
                if done {
                    return Ok(None);
                }
                let query = select.clone().for_page(page, size);
                let rows: Vec<M> = on_journey!(journey, |e| query.all(e).await)?;
                if rows.is_empty() {
                    return Ok(None);
                }
                let last = (rows.len() as u64) < size;
                Ok(Some((rows, (journey, select, page + 1, last))))
            },
        );
        Ok(Box::pin(windows))
    }

    /// The keyset windows of this (key-ordered) query: one
    /// [`cursor_paginate`](Self::cursor_paginate) per window on `journey`,
    /// each starting after the last window's last row.
    fn windows<'c>(self, journey: Journey<'c>, size: usize) -> Result<BoxStream<'c, Result<Vec<M>>>>
    where
        M: 'c,
    {
        if size == 0 {
            return Err(Error::InvalidQuery("`size` must be greater than 0".into()));
        }
        let size = size as u64;
        let windows = stream::try_unfold(
            (journey, self, None::<Cursor>, false),
            move |(mut journey, select, cursor, done)| async move {
                if done {
                    return Ok(None);
                }
                let query = select.clone();
                let page = on_journey!(journey, |e| query
                    .cursor_paginate(e, cursor.as_ref(), size)
                    .await)?;
                if page.items.is_empty() {
                    return Ok(None);
                }
                let last = page.next.is_none();
                Ok(Some((page.items, (journey, select, page.next, last))))
            },
        );
        Ok(Box::pin(windows))
    }
}

/// The rows of a stream of windows, one at a time.
fn flatten<'c, M: Send + 'c>(
    windows: impl futures_core::Stream<Item = Result<Vec<M>>> + Send + 'c,
) -> BoxStream<'c, Result<M>> {
    Box::pin(
        windows
            .map_ok(|rows| stream::iter(rows.into_iter().map(Ok)))
            .try_flatten(),
    )
}
