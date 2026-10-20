//! The pool statistics a `Db` reports.

/// Connection pool statistics, from [`Db::stats`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolStats {
    /// Open connections (idle and in use).
    pub size: u32,
    /// Idle connections.
    pub idle: usize,
    /// Configured maximum.
    pub max_connections: u32,
}
