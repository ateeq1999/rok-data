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

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use std::path::PathBuf;

use rok_data_sqlc::ApiFramework;

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
}

/// Nested actions of `rok generate`.
#[derive(Debug, Subcommand)]
pub(crate) enum GenerateAction {
    /// Fail when the generated files are out of date with the .sql files (for CI).
    Check {
        /// The subset of generation flags the check accepts. Provided leaf
        /// values win over any value given to `rok generate` itself.
        #[command(flatten)]
        options: CheckOptions,
    },
}

/// Nested actions of `rok gen`.
#[derive(Debug, Subcommand)]
pub(crate) enum GenAction {
    /// Fail when the generated files are out of date with the schema (for CI).
    Check {
        /// The subset of generation flags the check accepts. Provided leaf
        /// values win over any value given to `rok gen` itself.
        #[command(flatten)]
        options: GenCheckOptions,
    },
}

/// Options of `rok generate`.
#[derive(Debug, Args)]
pub(crate) struct GenerateOptions {
    /// Settings file (default: rok.toml, when it exists; legacy rok-data.toml accepted).
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

impl CheckOptions {
    /// The check's options over the generate run's: a value given to both
    /// goes to the leaf, a boolean given to either is set.
    pub(crate) fn over(self, group: GenerateOptions) -> GenerateOptions {
        GenerateOptions {
            config: self.config.or(group.config),
            input: self.input.or(group.input),
            output: self.output.or(group.output),
            crate_name: self.crate_name.or(group.crate_name),
            // The staging flags belong to the generate run; `check` refuses
            // them there.
            migration: None,
            renames: Vec::new(),
            allow_destructive: self.allow_destructive || group.allow_destructive,
            strict: self.strict || group.strict,
        }
    }
}

/// Options for the CI `check` variant: `--migration` and `--rename` are
/// deliberately absent, matching the original `rok-db-gen check` behavior.
#[derive(Debug, Args)]
pub(crate) struct CheckOptions {
    /// Settings file (default: rok.toml, when it exists; legacy rok-data.toml accepted).
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

/// Options of `rok gen`: the front door's flags.
#[derive(Debug, Args)]
pub(crate) struct GenOptions {
    /// Settings file (rok.toml, or the engine's rok-data-sqlc.toml).
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// The schema dir: the truth the generator reads (default: migrations).
    #[arg(short, long, value_name = "DIR")]
    pub schema: Option<PathBuf>,
    /// The migration history dir the staged pairs go in (default: migrations).
    #[arg(long, value_name = "DIR")]
    pub migrations_dir: Option<PathBuf>,
    /// Where the modules are written (default: src/db).
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<PathBuf>,
    /// Generate this table and the resources already generated.
    #[arg(short, long, value_name = "TABLE")]
    pub resource: Option<String>,
    /// Annotated query file or directory; repeatable.
    #[arg(short, long, value_name = "PATH")]
    pub queries: Vec<PathBuf>,
    /// The web framework the API features are generated for: `actix_web`
    /// (the default) or `axum`.
    #[arg(short = 'w', long, value_name = "FRAMEWORK", value_enum)]
    pub api_framework: Option<ApiFrameworkArg>,
    /// Stage a migration pair for the schema changes since the last one.
    #[arg(long, value_name = "NAME")]
    pub migration: Option<String>,
    /// Allow the steps that lose data or fail on existing rows.
    #[arg(long)]
    pub allow_destructive: bool,
    /// A renamed table (`users=accounts`) or column (`users.name=full_name`); repeatable.
    #[arg(long = "rename", value_name = "OLD=NEW")]
    pub renames: Vec<String>,
    /// Fail on warnings (a skipped table or query).
    #[arg(long)]
    pub strict: bool,
    /// Print a machine-readable JSON report.
    #[arg(long)]
    pub json: bool,
    /// Plan the run; write nothing.
    #[arg(short = 'n', long)]
    pub dry_run: bool,
    /// Overwrite files that the generator did not write.
    #[arg(short, long)]
    pub force: bool,
}

impl GenCheckOptions {
    /// The check's options over the generate run's: a value given to both
    /// goes to the leaf, a boolean given to either is set.
    pub(crate) fn over(self, group: GenOptions) -> GenOptions {
        GenOptions {
            config: self.config.or(group.config),
            schema: self.schema.or(group.schema),
            migrations_dir: self.migrations_dir.or(group.migrations_dir),
            output: self.output.or(group.output),
            resource: self.resource.or(group.resource),
            queries: if self.queries.is_empty() {
                group.queries
            } else {
                self.queries
            },
            api_framework: self.api_framework.or(group.api_framework),
            // The staging flags belong to the generate run; `check` refuses
            // them there.
            migration: None,
            allow_destructive: self.allow_destructive || group.allow_destructive,
            renames: Vec::new(),
            strict: self.strict || group.strict,
            json: self.json || group.json,
            dry_run: self.dry_run || group.dry_run,
            force: self.force || group.force,
        }
    }
}

/// Options of `rok gen check`: the same set minus `--migration` and
/// `--rename`, which belong to the generate run.
#[derive(Debug, Args)]
pub(crate) struct GenCheckOptions {
    /// Settings file (rok.toml, or the engine's rok-data-sqlc.toml).
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// The schema dir: the truth the generator reads (default: migrations).
    #[arg(short, long, value_name = "DIR")]
    pub schema: Option<PathBuf>,
    /// The migration history dir the staged pairs go in (default: migrations).
    #[arg(long, value_name = "DIR")]
    pub migrations_dir: Option<PathBuf>,
    /// Where the modules are written (default: src/db).
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<PathBuf>,
    /// Generate this table and the resources already generated.
    #[arg(short, long, value_name = "TABLE")]
    pub resource: Option<String>,
    /// Annotated query file or directory; repeatable.
    #[arg(short, long, value_name = "PATH")]
    pub queries: Vec<PathBuf>,
    /// The web framework the API features are generated for: `actix_web`
    /// (the default) or `axum`.
    #[arg(short = 'w', long, value_name = "FRAMEWORK", value_enum)]
    pub api_framework: Option<ApiFrameworkArg>,
    /// Allow the steps that lose data or fail on existing rows.
    #[arg(long)]
    pub allow_destructive: bool,
    /// Fail on warnings (a skipped table or query).
    #[arg(long)]
    pub strict: bool,
    /// Print a machine-readable JSON report.
    #[arg(long)]
    pub json: bool,
    /// Plan the run; write nothing.
    #[arg(short = 'n', long)]
    pub dry_run: bool,
    /// Overwrite files that the generator did not write.
    #[arg(short, long)]
    pub force: bool,
}

/// An `--api-framework` value. It mirrors [`ApiFramework`], which stays
/// free of clap because it is part of the stable library API
/// (engine ADR 0012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ApiFrameworkArg {
    /// axum: the routes are a `Router`'s, the handlers take axum's
    /// extractors.
    Axum,
    /// actix-web: the routes are a `web::ServiceConfig`'s scopes, the
    /// handlers take actix-web's extractors.
    #[value(name = "actix_web")]
    ActixWeb,
}

impl From<ApiFrameworkArg> for ApiFramework {
    fn from(arg: ApiFrameworkArg) -> Self {
        match arg {
            ApiFrameworkArg::Axum => ApiFramework::Axum,
            ApiFrameworkArg::ActixWeb => ApiFramework::ActixWeb,
        }
    }
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
    #[cfg(feature = "tools")]
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
