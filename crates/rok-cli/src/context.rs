//! Shared command plumbing: connect helpers and URL presentation.

use std::time::Duration;

use crate::config::{ServerConfig, read_servers};
use crate::keychain::get_password;

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

    /// Resolve the connection of a command that acts on a server:
    /// `--database-url`, else `--server`, else `DATABASE_URL` (ADR 0028).
    pub(crate) fn resolve_target(
        database_url: Option<&str>,
        server: Option<&str>,
    ) -> Result<Self, String> {
        match (database_url.filter(|u| !u.is_empty()), server) {
            (Some(url), _) => Self::resolve(Some(url)),
            (None, Some(server)) => Self::resolve_with_server(server),
            (None, None) => Self::resolve(None),
        }
    }

    /// Resolve connection using a registered server name.
    /// Builds the URL from servers.toml + password from keychain/env.
    pub(crate) fn resolve_with_server(server_name: &str) -> Result<Self, String> {
        let config = read_servers(None)?;
        let server = config
            .servers
            .iter()
            .find(|s| s.name == server_name)
            .ok_or_else(|| format!("server '{}' not found", server_name))?;

        let password = get_password(server_name).map_err(|e| e.to_string())?;

        let url = build_url_from_server(server, &password)?;
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

    /// One un-pooled connection, with the diagnostics every other connect
    /// gives: for work that needs a single session, such as a transaction.
    pub(crate) async fn connect(&self) -> Result<rok_data::sqlx::postgres::PgConnection, String> {
        self.probe().await
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

/// Builds a PostgreSQL connection URL from a ServerConfig and password.
fn build_url_from_server(server: &ServerConfig, password: &str) -> Result<String, String> {
    use url::Url;

    // Start with a base URL that has a host so we can set username/password
    let mut url =
        Url::parse("postgres://localhost").map_err(|e| format!("invalid base URL: {e}"))?;

    url.set_username(server.user.as_str())
        .map_err(|_| "invalid username")?;
    url.set_password(Some(password))
        .map_err(|_| "invalid password")?;

    let host = &server.host;
    url.set_host(Some(host)).map_err(|_| "invalid host")?;
    url.set_port(Some(server.port))
        .map_err(|_| "invalid port")?;

    // Use the server's database field (default: postgres) for admin operations
    let db = server.database.as_deref().unwrap_or("postgres");
    url.set_path(&format!("/{}", db));

    if let Some(sslmode) = &server.sslmode {
        url.query_pairs_mut().append_pair("sslmode", sslmode);
    }

    Ok(url.to_string())
}

/// Extracts the database name from a DATABASE_URL for project database protection.
pub(crate) fn extract_project_db_name(database_url: Option<&str>) -> Option<String> {
    database_url.and_then(|u| {
        url::Url::parse(u).ok().and_then(|url| {
            url.path_segments()
                .and_then(|mut s| s.next())
                .map(|s| s.to_owned())
        })
    })
}

/// Prompts the user to type a database name for confirmation.
/// Returns true if the input matches the expected name exactly.
#[allow(dead_code)]
pub(crate) fn confirm_drop_name(expected: &str) -> Result<bool, String> {
    use std::io::{Write, stdin, stdout};

    print!("Type the database name to confirm: ");
    stdout().flush().map_err(|e| e.to_string())?;

    let mut input = String::new();
    stdin().read_line(&mut input).map_err(|e| e.to_string())?;

    Ok(input.trim() == expected)
}
