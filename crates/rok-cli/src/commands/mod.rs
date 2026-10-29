//! Every command the `rok` binary implements, one module per group.

use std::process::ExitCode;

pub mod db;
pub mod r#gen;
pub mod generate;
pub mod init;
pub mod migrate;
pub mod schema;
pub mod sqlc;
#[cfg(feature = "tools")]
pub(crate) mod tools;

use crate::cli::{Command, DbAction, MigrateAction, SchemaAction};

/// Execute a parsed command; errors print `error: …` outside and exit 1.
pub(crate) async fn execute(command: Command) -> Result<ExitCode, String> {
    match command {
        Command::Init {
            name,
            template,
            api_framework,
            schema,
        } => init::run(name, template, api_framework.as_deref(), schema),
        Command::Generate {
            options: generation,
            action,
        } => generate::run(generation, action).await,
        Command::Gen {
            options: generation,
            action,
        } => r#gen::run(generation, action),
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
