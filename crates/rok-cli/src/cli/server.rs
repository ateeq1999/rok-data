//! `rok server add|list|remove`: the per-user server registry (ADR 0028).

use clap::Subcommand;

/// `rok server add|list|remove` — per-user server registry (ADR 0028).
#[derive(Debug, Subcommand)]
pub enum ServerAction {
    /// Register a new server.
    ///
    /// The password is extracted from the URL (if present) and stored in the OS keychain.
    /// If no keychain is available, a warning is printed and the user is instructed to set
    /// the ROK_SERVER_<NAME>_PASSWORD environment variable.
    Add {
        /// Name of the server (used as the keychain account).
        name: String,
        /// Connection URL: postgres://user:password@host:port/db?sslmode=...
        ///
        /// The password is extracted and stored in the keychain.
        #[arg(long, value_name = "URL", conflicts_with_all = ["host", "port", "user", "database", "sslmode", "password_prompt"])]
        url: Option<String>,
        /// Hostname or IP address.
        #[arg(long, value_name = "HOST", requires = "user")]
        host: Option<String>,
        /// Port number (default: 5432).
        #[arg(long, value_name = "PORT", default_value = "5432")]
        port: u16,
        /// Database user.
        #[arg(long, value_name = "USER")]
        user: Option<String>,
        /// Default database for admin operations (default: postgres).
        #[arg(long, value_name = "DB")]
        database: Option<String>,
        /// SSL mode: disable, allow, prefer, require, verify-ca, verify-full.
        #[arg(long, value_name = "MODE")]
        sslmode: Option<String>,
        /// Prompt for password interactively (hidden input).
        #[arg(long, conflicts_with = "url")]
        password_prompt: bool,
        /// Print machine-readable JSON output.
        #[arg(long)]
        json: bool,
    },
    /// List all registered servers.
    List {
        /// Print machine-readable JSON output.
        #[arg(long)]
        json: bool,
    },
    /// Remove a registered server and its password from the keychain.
    Remove {
        /// Name of the server to remove.
        name: String,
        /// Skip confirmation prompt.
        #[arg(long, short = 'y')]
        yes: bool,
        /// Print machine-readable JSON output.
        #[arg(long)]
        json: bool,
    },
}
