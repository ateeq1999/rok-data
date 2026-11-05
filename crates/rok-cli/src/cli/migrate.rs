//! `rok migrate create|status|up|down`.

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// `rok migrate create|status|up|down`.
#[derive(Debug, Subcommand)]
pub(crate) enum MigrateAction {
    /// Create a new numbered pair of up/down migration files.
    Create {
        /// Descriptive name, e.g. `create_posts_table`.
        name: String,
        #[command(flatten)]
        place: Place,
    },
    /// List every migration and its journal state.
    Status {
        #[command(flatten)]
        place: Place,
        /// The database to look at (default: DATABASE_URL).
        #[arg(long, value_name = "URL")]
        database_url: Option<String>,
        /// Print a machine-readable JSON object instead of the table.
        #[arg(long)]
        json: bool,
    },
    /// Apply every pending migration.
    Up {
        #[command(flatten)]
        place: Place,
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
        #[command(flatten)]
        place: Place,
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

/// Where a command finds the migration files: `--dir`, or the project's
/// history directory (`--app` picks a Monorepo's App).
#[derive(Debug, Args)]
pub(crate) struct Place {
    /// Directory of the migration files (default: the project's, found by
    /// walking up to the nearest `rok.toml`; else `migrations`).
    #[arg(long, value_name = "DIR")]
    pub(crate) dir: Option<PathBuf>,
    /// The Monorepo App to act on (default: the App you are in, or the only
    /// one `[apps]` names).
    #[arg(long, value_name = "NAME", conflicts_with = "dir")]
    pub(crate) app: Option<String>,
}
