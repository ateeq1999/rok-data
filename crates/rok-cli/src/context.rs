//! Shared command plumbing: connect helpers and URL presentation.

use std::time::Duration;

/// Every command that works on a database connects through here so the
/// timeout/error diagnostics match exactly (a plain probe first: a pooling
/// connect failure only says "timed out", the probe says why).
pub(crate) struct Connection {
    pub(crate) url: &'static str,
}

/// `DATABASE_URL` for commands that read it only for the generator's query
/// describe step (the value is allowed to be absent there).
pub(crate) fn env_database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty())
}

impl Connection {
    /// Resolve `--database-url`, else `DATABASE_URL`.
    pub(crate) fn resolve(flag: Option<&str>) -> Result<Self, String> {
        let url = flag
            .filter(|u| !u.is_empty())
            .map(str::to_owned)
            .or_else(env_database_url)
            .ok_or_else(|| {
                "needs a database: set DATABASE_URL or pass --database-url".to_owned()
            })?;
        Ok(Connection {
            url: Box::leak(url.into_boxed_str()),
        })
    }

    /// The URL safe to echo: userinfo and password dropped, query string
    /// (which may carry `password=`) dropped.
    pub(crate) fn url_masked(&self) -> String {
        let url = self.url;
        let rest = url.split_once("://").map_or(url.to_owned(), |(scheme, r)| {
            let host_part = match r.find('@') {
                // user:password@host — keep scheme://host…
                Some(at) => &r[at + 1..],
                None => r,
            };
            format!("{scheme}://{host_part}")
        });
        match rest.find('?') {
            Some(q) => rest[..q].to_owned(),
            None => rest,
        }
    }

    /// One un-pooled connection to surface reachability/auth errors fast.
    async fn probe(&self) -> Result<rok_data::sqlx::postgres::PgConnection, String> {
        use rok_data::sqlx::Connection as SqlxConnection;
        let timeout = Duration::from_secs(10);
        tokio::time::timeout(timeout, rok_data::sqlx::PgConnection::connect(self.url))
            .await
            .map_err(|_| "can't connect to the database: no answer within 10 seconds".to_owned())?
            .map_err(|e| format!("can't connect to the database: {e}"))
    }

    /// A one-connection `Db` for command work that must not hog a pool.
    pub(crate) async fn probe_into_db(&self) -> Result<rok_data::Db, String> {
        let timeout = Duration::from_secs(10);
        let _ = self.probe().await?;
        rok_data::Db::builder()
            .max_connections(1)
            .acquire_timeout(timeout)
            .connect(self.url)
            .await
            .map_err(|e| format!("can't connect to the database: {e}"))
    }
}
