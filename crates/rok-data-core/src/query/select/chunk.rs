//! The windowed read: [`Select::chunk`] walks the matching rows in windows,
//! so a table that does not fit in memory can still be processed.

use std::future::Future;

use crate::model::Model;
use crate::{Error, Executor, Result, TryStreamExt};

use super::Select;

impl<M: Model> Select<M> {
    /// Process the matching rows in windows of `size`, handing each window to
    /// `f`.
    ///
    /// The rows are streamed, not loaded: only one window is in memory at a
    /// time, and the query runs once — not once per window. Every matching
    /// row is handed to `f` exactly once, in the order the query returns
    /// them; the last window is shorter when the rows don't divide by
    /// `size`.
    ///
    /// `f` returns a future, so a window can be written back to the database
    /// before the next one arrives. The first error stops the walk and is
    /// returned.
    ///
    /// ```ignore
    /// User::query().chunk(&db, 100, |rows| async move {
    ///     for user in rows {
    ///         archive(&db, user).await?;
    ///     }
    ///     Ok(())
    /// }).await?;
    /// ```
    ///
    /// The future must own what it needs (an `async move` block): it cannot
    /// borrow the closure's environment.
    ///
    /// The query holds its connection while the windows are processed, so a
    /// callback that writes back needs a second one: give the pool more than
    /// one connection.
    pub async fn chunk<'e, E, F, Fut>(self, executor: E, size: usize, mut f: F) -> Result<()>
    where
        E: Executor<'e> + 'e,
        F: FnMut(Vec<M>) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        if size == 0 {
            return Err(Error::InvalidQuery("`size` must be greater than 0".into()));
        }
        let mut rows = self.stream(executor);
        // The window grows as the rows arrive: a large `size` costs nothing
        // until the table fills it.
        let mut window: Vec<M> = Vec::new();
        while let Some(row) = rows.try_next().await? {
            window.push(row);
            if window.len() == size {
                f(std::mem::take(&mut window)).await?;
            }
        }
        if !window.is_empty() {
            f(window).await?;
        }
        Ok(())
    }
}
