//! Every command the `rok` binary implements, one module per group.

use std::process::ExitCode;

pub mod db;
pub mod r#gen;
pub mod generate;
pub mod init;
pub mod migrate;
mod place;
pub mod schema;
pub mod server;
pub mod sqlc;

use crate::cli::Command;
use crate::cli::db::DbAction;
use crate::cli::migrate::MigrateAction;
use crate::cli::schema::SchemaAction;

/// Execute a parsed command; errors print `error: …` outside and exit 1.
pub(crate) async fn execute(command: Command) -> Result<ExitCode, String> {
    match command {
        Command::Init {
            name,
            template,
            org,
            api_framework,
            schema,
        } => init::run(
            name,
            template,
            org.as_deref(),
            api_framework.as_deref(),
            schema,
        ),
        Command::Generate {
            options: generation,
            action,
        } => generate::run(generation, action).await,
        Command::Gen {
            options: generation,
            action,
        } => r#gen::run(generation, action),
        Command::Migrate(action) => match action {
            MigrateAction::Create { name, place } => {
                migrate::create(&name, &place::migrations_dir(&place)?)
            }
            MigrateAction::Status {
                place,
                database_url,
                json,
            } => {
                migrate::status(
                    &place::migrations_dir(&place)?,
                    database_url.as_deref(),
                    json,
                )
                .await
            }
            MigrateAction::Up {
                place,
                database_url,
                json,
            } => {
                migrate::up(
                    &place::migrations_dir(&place)?,
                    database_url.as_deref(),
                    json,
                )
                .await
            }
            MigrateAction::Down {
                place,
                database_url,
                steps,
                json,
            } => {
                migrate::down(
                    &place::migrations_dir(&place)?,
                    database_url.as_deref(),
                    steps,
                    json,
                )
                .await
            }
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
            DbAction::Create {
                name,
                database_url,
                server,
                json,
            } => db::create(&name, database_url.as_deref(), server.as_deref(), json).await,
            DbAction::Drop {
                name,
                database_url,
                server,
                yes,
                force,
                json,
            } => {
                db::drop(
                    &name,
                    database_url.as_deref(),
                    server.as_deref(),
                    yes,
                    force,
                    json,
                )
                .await
            }
            DbAction::Query {
                sql,
                database_url,
                server,
                json,
            } => db::query(&sql, database_url.as_deref(), server.as_deref(), json).await,
        },
        Command::Schema(action) => match action {
            SchemaAction::Dump { database_url } => schema::dump(database_url.as_deref()).await,
            SchemaAction::Diff {
                place,
                database_url,
            } => schema::diff(&place::migrations_dir(&place)?, database_url.as_deref()).await,
        },
        Command::Server(action) => server::run(action),
    }
}
