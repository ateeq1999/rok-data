//! The flag values of the command line, each mirroring a library type that
//! stays free of clap because it is part of the stable library API (engine
//! ADR 0012).

use crate::queries::ReturnMode;
use crate::{ApiFramework, Derive, Extractor};

/// The return mode of `query add`, when the SQL doesn't say it the way the
/// inference reads it: an `INSERT … RETURNING` is `:one`, a `SELECT` or other
/// `RETURNING` is `:many` (`:maybe_one` with `LIMIT 1`), anything else
/// `:execrows`.
#[derive(Debug, clap::Args)]
#[group(multiple = false)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "clap flags of one exclusive group"
)]
pub(super) struct ModeArgs {
    /// Exactly one row (`:one`)
    #[arg(short = 'o', long)]
    one: bool,
    /// Zero or one row (`:maybe_one`)
    #[arg(short, long)]
    maybe_one: bool,
    /// Any number of rows (`:many`)
    #[arg(short = 'M', long)]
    many: bool,
    /// No result (`:exec`)
    #[arg(short = 'x', long)]
    exec: bool,
    /// The number of affected rows (`:execrows`)
    #[arg(short = 'r', long)]
    execrows: bool,
}

impl ModeArgs {
    pub(super) fn mode(&self) -> Option<ReturnMode> {
        [
            (self.one, ReturnMode::One),
            (self.maybe_one, ReturnMode::MaybeOne),
            (self.many, ReturnMode::Many),
            (self.exec, ReturnMode::Exec),
            (self.execrows, ReturnMode::ExecRows),
        ]
        .into_iter()
        .find_map(|(set, mode)| set.then_some(mode))
    }
}

/// A `--derive` value. It mirrors [`Derive`], which stays free of clap
/// because it is part of the stable library API (engine ADR 0012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(super) enum DeriveArg {
    Serde,
    Validate,
}

impl From<DeriveArg> for Derive {
    fn from(arg: DeriveArg) -> Self {
        match arg {
            DeriveArg::Serde => Derive::Serde,
            DeriveArg::Validate => Derive::Validate,
        }
    }
}

/// An `--extractor` value, mirroring [`Extractor`] as [`DeriveArg`]
/// mirrors [`Derive`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(super) enum ExtractorArg {
    Axum,
    #[value(name = "actix_web")]
    ActixWeb,
}

impl From<ExtractorArg> for Extractor {
    fn from(arg: ExtractorArg) -> Self {
        match arg {
            ExtractorArg::Axum => Extractor::Axum,
            ExtractorArg::ActixWeb => Extractor::ActixWeb,
        }
    }
}

/// An `--api-framework` value. It mirrors [`ApiFramework`], which stays
/// free of clap because it is part of the stable library API (engine ADR 0012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(super) enum FrameworkArg {
    Axum,
    #[value(name = "actix_web")]
    ActixWeb,
}

impl From<FrameworkArg> for ApiFramework {
    fn from(arg: FrameworkArg) -> Self {
        match arg {
            FrameworkArg::Axum => ApiFramework::Axum,
            FrameworkArg::ActixWeb => ApiFramework::ActixWeb,
        }
    }
}
