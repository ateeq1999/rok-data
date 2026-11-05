//! `rok schema dump|diff`.

use clap::Subcommand;

use super::migrate::Place;

/// `rok schema dump|diff`.
#[derive(Debug, Subcommand)]
pub(crate) enum SchemaAction {
    /// Dump every user table as approximate CREATE TABLE DDL.
    Dump {
        /// The database to dump (default: DATABASE_URL).
        #[arg(long, value_name = "URL")]
        database_url: Option<String>,
    },
    /// Compare the migration journal with the files in --dir.
    Diff {
        #[command(flatten)]
        place: Place,
        /// The database (default: DATABASE_URL).
        #[arg(long, value_name = "URL")]
        database_url: Option<String>,
    },
}
