//! `rok migrate create|status|up|down`.

use clap::Subcommand;
use std::path::PathBuf;

/// `rok migrate create|status|up|down`.
#[derive(Debug, Subcommand)]
pub(crate) enum MigrateAction {
    /// Create a new numbered pair of up/down migration files.
    Create {
        /// Descriptive name, e.g. `create_posts_table`.
        name: String,
        /// Directory of the migration files.
        #[arg(long, value_name = "DIR", default_value = "migrations")]
        dir: PathBuf,
    },
    /// List every migration and its journal state.
    Status {
        /// Directory of the migration files.
        #[arg(long, value_name = "DIR", default_value = "migrations")]
        dir: PathBuf,
        /// The database to look at (default: DATABASE_URL).
        #[arg(long, value_name = "URL")]
        database_url: Option<String>,
        /// Print a machine-readable JSON object instead of the table.
        #[arg(long)]
        json: bool,
    },
    /// Apply every pending migration.
    Up {
        /// Directory of the migration files.
        #[arg(long, value_name = "DIR", default_value = "migrations")]
        dir: PathBuf,
        /// The database to migrate (default: DATABASE_URL).
        #[arg(long, value_name = "URL")]
        database_url: Option<String>,
        /// Print a machine-readable JSON object instead of a list.
        #[arg(long)]
        json: bool,
    },
    /// Revert the latest migration; also readable as `rok migrate rollback`.
    #[command(alias = "rollback")]
    Down {
        /// Directory of the migration files.
        #[arg(long, value_name = "DIR", default_value = "migrations")]
        dir: PathBuf,
        /// The database to migrate (default: DATABASE_URL).
        #[arg(long, value_name = "URL")]
        database_url: Option<String>,
        /// How many migrations to revert.
        #[arg(long, value_name = "N", default_value_t = 1)]
        steps: usize,
        /// Print a machine-readable JSON object instead of a list.
        #[arg(long)]
        json: bool,
    },
}
