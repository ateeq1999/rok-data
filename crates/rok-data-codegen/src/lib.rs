//! Generate a rok-data crate from `.sql` files: models and enums from
//! `CREATE TABLE` / `CREATE TYPE`, typed functions from sqlc-style annotated
//! queries, and migrations computed from schema changes.
//!
//! Most people use the `rok` binary; see the crate README.

mod ddl;
mod describe;
mod emit;
mod generate;
mod ir;
mod naming;
mod parse;
mod queries;
mod split;
mod staging;
mod truth;
mod types;

pub use ddl::Rename;
pub use generate::{Dependency, Generated, Options, differences, generate, write};
pub use ir::{Column, EnumType, Extra, ForeignKey, Index, Query, QueryKind, Schema, Table, Unique};
pub use parse::{Parsed, SourceFile, parse};
pub use staging::{StageOptions, Staged, snapshot_path, stage, truth_files};
pub use truth::{MigrationFile, MigrationPair, existing_migrations, migration, schema};
pub use types::canonical as canonical_sql_type;

use std::fmt;

/// A generation error, with the file and line when known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
    nothing_to_migrate: bool,
}

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            nothing_to_migrate: false,
        }
    }

    pub(crate) fn at(location: &str, message: impl fmt::Display) -> Self {
        Self::new(format!("{location}: {message}"))
    }

    /// The migration staging's deliberate stop: the diff of the truth
    /// against the state of the last staged migration is empty, so the
    /// pair to write is empty. [`Error::new`] builds a plain failure;
    /// this constructor owns the message, so its exact wording is the
    /// one to pin and callers branch on [`Error::is_nothing_to_migrate`]
    /// instead of matching the text.
    pub(crate) fn nothing_to_migrate() -> Self {
        Self {
            message: "the schema hasn't changed since the last migration; nothing to migrate"
                .to_owned(),
            nothing_to_migrate: true,
        }
    }

    /// Whether the staging found nothing to migrate: the truth equals the
    /// state of the last staged migration, no pair was written, and a
    /// caller may treat the run as the clean state instead of a failure.
    pub fn is_nothing_to_migrate(&self) -> bool {
        self.nothing_to_migrate
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}
