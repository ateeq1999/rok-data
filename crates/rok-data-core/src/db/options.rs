//! The transaction options: isolation level, read-only, and retries.

use std::time::Duration;

/// A transaction's isolation level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Isolation {
    /// `READ COMMITTED` (PostgreSQL's default).
    ReadCommitted,
    /// `REPEATABLE READ`.
    RepeatableRead,
    /// `SERIALIZABLE`: transactions behave as if run one after another;
    /// conflicting ones fail with a serialization error (and are retried by
    /// [`Db::transaction_with`](crate::Db::transaction_with)).
    Serializable,
}

/// Settings for [`Db::transaction_with`](crate::Db::transaction_with).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxOptions {
    isolation: Option<Isolation>,
    read_only: bool,
    pub(crate) retries: u32,
    base_delay: Duration,
}

impl Default for TxOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl TxOptions {
    /// Default settings: the server's isolation level, read-write, no retries.
    pub fn new() -> Self {
        Self {
            isolation: None,
            read_only: false,
            retries: 0,
            base_delay: Duration::from_millis(10),
        }
    }

    /// Run with this isolation level.
    pub fn isolation(mut self, isolation: Isolation) -> Self {
        self.isolation = Some(isolation);
        self
    }

    /// Run as `READ ONLY`.
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    /// Retry up to `retries` times on serialization failures and deadlocks.
    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// Base delay of the exponential backoff between retries (default 10ms;
    /// attempt *n* waits `base * 2^(n-1)` plus up to 50% jitter, capped at 1s).
    pub fn backoff_base(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
    }

    pub(crate) fn backoff(&self, attempt: u32) -> Duration {
        let exp = self
            .base_delay
            .saturating_mul(1 << attempt.saturating_sub(1).min(16))
            .min(Duration::from_secs(1));
        // Cheap jitter without a RNG dependency.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        exp + exp.mul_f64(f64::from(nanos % 500) / 1000.0)
    }

    pub(crate) fn set_transaction_sql(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(isolation) = self.isolation {
            parts.push(match isolation {
                Isolation::ReadCommitted => "ISOLATION LEVEL READ COMMITTED",
                Isolation::RepeatableRead => "ISOLATION LEVEL REPEATABLE READ",
                Isolation::Serializable => "ISOLATION LEVEL SERIALIZABLE",
            });
        }
        if self.read_only {
            parts.push("READ ONLY");
        }
        (!parts.is_empty()).then(|| format!("SET TRANSACTION {}", parts.join(", ")))
    }
}
