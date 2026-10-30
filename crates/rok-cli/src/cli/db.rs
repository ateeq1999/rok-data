//! `rok db check|inspect|create|drop`.

use clap::Subcommand;

/// `rok db check|inspect|create|drop`.
#[derive(Debug, Subcommand)]
pub(crate) enum DbAction {
    /// Connect, ping, and report server version plus pool settings.
    Check {
        /// The database to check (default: DATABASE_URL).
        #[arg(long, value_name = "URL")]
        database_url: Option<String>,
        /// Print a machine-readable JSON object instead of prose.
        #[arg(long)]
        json: bool,
    },
    /// List user tables, or (--table) the columns, indexes and foreign keys of one.
    Inspect {
        /// Restrict the output to one table.
        #[arg(long, value_name = "TABLE")]
        table: Option<String>,
        /// The database to inspect (default: DATABASE_URL).
        #[arg(long, value_name = "URL")]
        database_url: Option<String>,
        /// Print a machine-readable JSON object.
        #[arg(long)]
        json: bool,
    },
    /// Create a new database on the target server.
    Create {
        /// Name of the database to create.
        name: String,
        /// The database to connect to (default: DATABASE_URL).
        #[arg(long, value_name = "URL")]
        database_url: Option<String>,
        /// Use a registered server (default: DATABASE_URL).
        #[arg(long, value_name = "NAME")]
        server: Option<String>,
        /// Print a machine-readable JSON object.
        #[arg(long)]
        json: bool,
    },
    /// Drop a database on the target server.
    Drop {
        /// Name of the database to drop.
        name: String,
        /// The database to connect to (default: DATABASE_URL).
        #[arg(long, value_name = "URL")]
        database_url: Option<String>,
        /// Use a registered server (default: DATABASE_URL).
        #[arg(long, value_name = "NAME")]
        server: Option<String>,
        /// Skip the typed-name confirmation prompt (for scripts).
        #[arg(long, short = 'y')]
        yes: bool,
        /// Allow dropping the project's own database (from DATABASE_URL).
        #[arg(long)]
        force: bool,
        /// Print a machine-readable JSON object.
        #[arg(long)]
        json: bool,
    },
}
