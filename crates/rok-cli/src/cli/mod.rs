//! The `rok` command line: parsing, help text, and usage errors.
//!
//! `rok sqlc …` is defined by the `rok-data-sqlc` engine (see
//! [`crate::commands::sqlc`]) so its grammar and its tests share one source;
//! everything below owns the mapping into library calls.
//!
//! `rok generate` and `rok sqlc …` are the legacy surfaces
//! (workspace ADR 0012): their flags, their output and their exit
//! codes are unchanged — `rok sqlc …` is forwarded verbatim to the
//! engine, and `rok generate` no longer takes the `generate`
//! subcommand — and the root help marks them as superseded by the
//! front door (`rok init`, `rok gen`), their removal deferred to 1.0.

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use std::path::PathBuf;

pub(crate) mod db;
pub(crate) mod r#gen;
pub(crate) mod generate;
pub(crate) mod migrate;
pub(crate) mod schema;
pub(crate) mod server;

use db::DbAction;
use r#gen::{GenAction, GenOptions};
use generate::{GenerateAction, GenerateOptions};
use migrate::MigrateAction;
use schema::SchemaAction;
use server::ServerAction;

/// Root of the command line.
#[derive(Debug, Parser)]
#[command(
    name = "rok",
    about = "Database tooling for the rok-data ecosystem",
    version,
    arg_required_else_help = true,
    disable_help_subcommand = true,
    after_help = "Most commands accept --json for machine-readable output; \
                  `rok init --help` scaffolds a project; \
                  `rok gen --help` generates the data layer from the schema dir; \
                  `rok generate` and `rok sqlc` are the legacy surfaces: still \
                  supported and forwarded verbatim, superseded by `rok init`/`rok gen`, \
                  removal deferred to 1.0."
)]
pub(crate) struct Cli {
    /// Selected command.
    #[command(subcommand)]
    pub command: Command,
}

/// Every `rok` command group.
///
// `large_enum_variant` is allowed on purpose: the `generate` variant carries
// the generator's flag set inline (clap cannot flatten a `Box`), the enum is
// built once per process run, and its work afterwards is I/O that dwarfs
// a few hundred bytes.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Scaffold a project: the application skeleton, or a ready-made template.
    Init {
        /// The project directory (default: the current directory).
        #[arg(value_name = "NAME")]
        name: Option<PathBuf>,
        /// Copy this ready-made template instead of the skeleton.
        #[arg(
            short,
            long,
            value_name = "NAME",
            value_parser = clap::builder::PossibleValuesParser::new(
                rok_data_sqlc::init::template_names()
            )
        )]
        template: Option<String>,
        /// The web framework the skeleton is written for: `actix_web` (the
        /// default) or `axum`. A template has its own, so this doesn't
        /// combine with `--template`.
        #[arg(
            short = 'w',
            long,
            value_name = "FW",
            conflicts_with = "template",
            value_parser = ["actix_web", "axum"]
        )]
        api_framework: Option<String>,
        /// Seed the schema directory (migrations/) with this file's content.
        #[arg(short, long, value_name = "PATH")]
        schema: Option<PathBuf>,
    },
    /// Legacy: generate a rok-data crate (models, typed queries,
    /// migrations) from .sql files; superseded by `rok gen`.
    Generate {
        /// What and where to generate; the flags of the original generator.
        #[command(flatten)]
        options: GenerateOptions,
        /// Optional action: the CI mode lives nested (as `rok generate check`) so
        /// the write mode keeps its flat `rok generate [OPTIONS]` shape.
        #[command(subcommand)]
        action: Option<GenerateAction>,
    },
    /// Generate the data layer and the API features from the schema dir.
    Gen {
        /// What and where to generate; the flags of the front door.
        #[command(flatten)]
        options: GenOptions,
        /// Optional action: the CI mode lives nested (as `rok gen check`) so
        /// the write mode keeps its flat `rok gen [OPTIONS]` shape.
        #[command(subcommand)]
        action: Option<GenAction>,
    },
    /// Run migration files against the database.
    #[command(subcommand)]
    Migrate(MigrateAction),
    /// Connectivity and structure of the configured database.
    #[command(subcommand)]
    Db(DbAction),
    /// The live schema as DDL, and journal-vs-files comparison.
    #[command(subcommand)]
    Schema(SchemaAction),
    /// Per-user server registry (ADR 0028).
    #[command(subcommand)]
    Server(ServerAction),
}

/// How [`parse`] finished.
/// What the root parser decided: run a command, print help/version, or
/// report usage.
// Carries [`Command`], whose size it inherits; see the allow note there.
#[allow(clippy::large_enum_variant)]
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
    let command = Cli::command().subcommand(rok_data_sqlc::cli::command().name("sqlc").about(
        "Legacy: generate SQLx Rust modules from SQL schema files \
                 (generate|init|check-arch); superseded by `rok gen`",
    ));
    // Likewise `rok tools …` is routed to rok-tools before this parser (ADR 0026).
    #[cfg(rok_tools)]
    let command = command.subcommand(
        rok_tools_cli::command()
            .name("tools")
            .about("Developer tools: grep, replace, loc, refactor, … (`rok tools list`)"),
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
