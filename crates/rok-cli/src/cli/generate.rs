//! `rok generate` (legacy): its options and its nested `check` action.

use clap::{Args, Subcommand};
use std::path::PathBuf;

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
