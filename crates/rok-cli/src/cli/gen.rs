//! `rok gen` (the front door): its options and its nested `check` action.

use clap::{Args, Subcommand};
use std::path::PathBuf;

use rok_data_sqlc::ApiFramework;

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
