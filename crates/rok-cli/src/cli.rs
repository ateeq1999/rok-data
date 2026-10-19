//! The `rok` command line: parsing, help text, and usage errors.
//!
//! `rok sqlc …` is defined by the `rok-data-sqlc` engine (see
//! [`crate::commands::sqlc`]) so its grammar and its tests share one source;
//! everything below owns the mapping into library calls.

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use std::path::PathBuf;

/// Root of the command line.
#[derive(Debug, Parser)]
#[command(
    name = "rok",
    about = "Database tooling for the rok-data ecosystem",
    version,
    arg_required_else_help = true,
    disable_help_subcommand = true,
    after_help = "Most commands accept --json for machine-readable output; \
                  `rok sqlc --help` is the SQLx module generator (generate|init|check-arch)."
)]
pub(crate) struct Cli {
    /// Selected command.
    #[command(subcommand)]
    pub command: Command,
}

/// Every `rok` command group.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Generate a rok-data crate (models, typed queries, migrations) from .sql files.
    #[command(subcommand)]
    Generate(GenerateAction),
    /// Run migration files against the database.
    #[command(subcommand)]
    Migrate(MigrateAction),
    /// Connectivity and structure of the configured database.
    #[command(subcommand)]
    Db(DbAction),
    /// The live schema as DDL, and journal-vs-files comparison.
    #[command(subcommand)]
    Schema(SchemaAction),
}

/// `rok generate` and `rok generate check`.
#[derive(Debug, Subcommand)]
pub(crate) enum GenerateAction {
    /// Write the generated crate (from the .sql files) to the output directory.
    Generate(GenerateOptions),
    /// Fail when the generated files are out of date with the .sql files (for CI).
    Check(CheckOptions),
}

/// Options of `rok generate`.
#[derive(Debug, Args)]
pub(crate) struct GenerateOptions {
    /// Settings file (default: rok-data.toml, when it exists).
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Directory holding the .sql files (default: db).
    #[arg(short, long, value_name = "DIR")]
    pub input: Option<PathBuf>,
    /// Generated crate directory (default: db-gen).
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<PathBuf>,
    /// Package name of the generated crate (default: db-gen).
    #[arg(long, value_name = "NAME")]
    pub crate_name: Option<String>,
    /// Add a migration for the schema changes since the last generation.
    #[arg(long, value_name = "NAME")]
    pub migration: Option<String>,
    /// A renamed table (`users=accounts`) or column (`users.name=full_name`); repeatable.
    #[arg(long = "rename", value_name = "OLD=NEW")]
    pub renames: Vec<String>,
    /// Allow steps that drop data or can fail on existing rows.
    #[arg(long)]
    pub allow_destructive: bool,
    /// Fail on warnings (queries without tenant or soft-delete filters).
    #[arg(long)]
    pub strict: bool,
}

/// Options for the CI `check` variant: `--migration` and `--rename` are
/// deliberately absent, matching the original `rok-db-gen check` behavior.
#[derive(Debug, Args)]
pub(crate) struct CheckOptions {
    /// Settings file (default: rok-data.toml, when it exists).
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Directory holding the .sql files (default: db).
    #[arg(short, long, value_name = "DIR")]
    pub input: Option<PathBuf>,
    /// Generated crate directory (default: db-gen).
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<PathBuf>,
    /// Package name of the generated crate (default: db-gen).
    #[arg(long, value_name = "NAME")]
    pub crate_name: Option<String>,
    /// Allow steps that drop data or can fail on existing rows.
    #[arg(long)]
    pub allow_destructive: bool,
    /// Fail on warnings (queries without tenant or soft-delete filters).
    #[arg(long)]
    pub strict: bool,
}

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

/// `rok db check|inspect`.
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
}

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
        /// Directory of the migration files.
        #[arg(long, value_name = "DIR", default_value = "migrations")]
        dir: PathBuf,
        /// The database (default: DATABASE_URL).
        #[arg(long, value_name = "URL")]
        database_url: Option<String>,
    },
}

/// How [`parse`] finished.
pub(crate) enum ParseOutcome {
    /// A command to execute.
    Command(Command),
    /// `--help` / `--version`; already printed to the right stream.
    Informational,
    /// Usage error; clap's message already printed to stderr, exit 2.
    Usage,
}

/// Parse `args` (which must NOT include the program name) into a command.
///
/// The `sqlc` subtree (owned by the `rok-data-sqlc` engine) is injected into
/// the command tree only so the root help lists it; tokens after `rok sqlc`
/// never reach this parser (see [`crate::route_sqlc`]).
pub(crate) fn parse(args: &[String]) -> ParseOutcome {
    // The `sqlc` subtree (defined by the engine) is only injected so the root
    // help lists it; `rok sqlc …` tokens are routed before this parser runs.
    let command = Cli::command().subcommand(
        rok_data_sqlc::cli::command()
            .name("sqlc")
            .about("Generate SQLx Rust modules from SQL schema files (generate|init|check-arch)"),
    );
    match command.try_get_matches_from(inputs(args)) {
        Ok(matches) => match Command::from_arg_matches(&matches) {
            Ok(command) => ParseOutcome::Command(command),
            Err(_) => ParseOutcome::Usage,
        },
        Err(err) => {
            if matches!(
                err.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                if err.use_stderr() {
                    eprint!("{err}");
                } else {
                    print!("{err}");
                }
                ParseOutcome::Informational
            } else {
                eprint!("{err}");
                ParseOutcome::Usage
            }
        }
    }
}

/// The tokens behind parse: the program name, then everything the user typed.
fn inputs(args: &[String]) -> Vec<String> {
    let mut all = vec!["rok".to_owned()];
    all.extend(args.iter().cloned());
    all
}
