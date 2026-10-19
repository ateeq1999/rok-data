//! Every command the `rok` binary implements, one module per group.

use std::process::ExitCode;

pub mod db;
pub mod generate;
pub mod migrate;
pub mod schema;
pub mod sqlc;

use crate::cli::{Command, DbAction, GenerateAction, MigrateAction, SchemaAction};

/// Execute a parsed command; errors print `error: …` outside and exit 1.
pub(crate) async fn execute(command: Command) -> Result<ExitCode, String> {
    match command {
        Command::Generate(action) => match action {
            GenerateAction::Generate(options) => generate::generate(options).await,
            GenerateAction::Check(options) => generate::check(options).await,
        },
        Command::Migrate(action) => match action {
            MigrateAction::Create { name, dir } => migrate::create(&name, &dir),
            MigrateAction::Status {
                dir,
                database_url,
                json,
            } => migrate::status(&dir, database_url.as_deref(), json).await,
            MigrateAction::Up {
                dir,
                database_url,
                json,
            } => migrate::up(&dir, database_url.as_deref(), json).await,
            MigrateAction::Down {
                dir,
                database_url,
                steps,
                json,
            } => migrate::down(&dir, database_url.as_deref(), steps, json).await,
        },
        Command::Db(action) => match action {
            DbAction::Check { database_url, json } => {
                db::check(database_url.as_deref(), json).await
            }
            DbAction::Inspect {
                table,
                database_url,
                json,
            } => db::inspect(table.as_deref(), database_url.as_deref(), json).await,
        },
        Command::Schema(action) => match action {
            SchemaAction::Dump { database_url } => schema::dump(database_url.as_deref()).await,
            SchemaAction::Diff { dir, database_url } => {
                schema::diff(&dir, database_url.as_deref()).await
            }
        },
    }
}
