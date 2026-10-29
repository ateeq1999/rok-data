//! `check --db`: every annotated query prepared against a live database,
//! its parameter and column types compared with the ones the generated code
//! binds and decodes (engine ADR 0026).
//!
//! The engine never connects: a [`Describer`] does, handed in by the binary
//! (`rok`, through `rok_data::introspect::describe`), so the engine keeps no
//! database driver.

mod types;

use std::fmt;
use std::path::PathBuf;

use crate::generate::{Analysis, LocatedQuery};
use crate::queries::ResultShape;
use crate::report::display_path;
use types::Known;

/// Prepares SQL against a database and answers its types, without running
/// it.
pub trait Describer {
    /// The parameter and column types of `sql`, or the database's error
    /// when it does not prepare.
    fn describe(&mut self, sql: &str) -> Result<Described, String>;
}

/// A statement's types as the database reports them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Described {
    /// Each parameter's type name, in placeholder order (`INT8`, `TEXT[]`,
    /// or an enum's own name).
    pub params: Vec<String>,
    /// Each result column, in order.
    pub columns: Vec<DescribedColumn>,
}

/// One result column of a [`Described`] statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribedColumn {
    pub name: String,
    pub type_name: String,
    /// Whether it can be `NULL`; `None` when the database can't tell.
    pub nullable: Option<bool>,
}

/// One difference between a query's generated code and the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mismatch {
    pub(crate) path: PathBuf,
    pub(crate) line: usize,
    pub(crate) query: String,
    pub(crate) message: String,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {}: {}",
            display_path(&self.path),
            self.line,
            self.query,
            self.message
        )
    }
}

/// What [`check`] found.
#[derive(Debug, Default)]
pub(crate) struct Outcome {
    pub(crate) checked: usize,
    pub(crate) mismatches: Vec<Mismatch>,
    /// Values whose Rust type the check doesn't know (a user's own
    /// `-- param:` type, say): `query: what`, left unchecked.
    pub(crate) unchecked: Vec<String>,
}

/// Prepares every query of `analysis` with `describer` and compares.
pub(crate) fn check(analysis: &Analysis, describer: &mut dyn Describer) -> Outcome {
    let known = Known::new(&analysis.tables, &analysis.enums);
    let mut outcome = Outcome::default();
    for located in &analysis.queries {
        outcome.checked += 1;
        let mut found = Found {
            located,
            outcome: &mut outcome,
        };
        match describer.describe(&located.query.sql) {
            Ok(described) => found.compare(&described, analysis, &known),
            Err(error) => found.mismatch(format!("does not prepare: {error}")),
        }
    }
    outcome
}

/// The differences of one query, recorded into the run's [`Outcome`].
struct Found<'a> {
    located: &'a LocatedQuery,
    outcome: &'a mut Outcome,
}

impl Found<'_> {
    fn mismatch(&mut self, message: String) {
        self.outcome.mismatches.push(Mismatch {
            path: self.located.path.clone(),
            line: self.located.line,
            query: self.located.query.name.clone(),
            message,
        });
    }

    fn unchecked(&mut self, what: String) {
        let query = &self.located.query.name;
        self.outcome.unchecked.push(format!("{query}: {what}"));
    }

    fn compare(&mut self, described: &Described, analysis: &Analysis, known: &Known) {
        let query = &self.located.query;
        if described.params.len() != query.params.len() {
            self.mismatch(format!(
                "takes {} parameter(s) in the database; the generated function binds {}",
                described.params.len(),
                query.params.len()
            ));
        } else {
            for (param, db_type) in query.params.iter().zip(&described.params) {
                let what = format!("parameter `${}` (`{}`)", param.index, param.name);
                self.compare_type(&what, db_type, &param.rust_type, known);
            }
        }
        for (index, expected) in expected_columns(&query.result, analysis).iter().enumerate() {
            let Some(column) = described.columns.get(index) else {
                break;
            };
            let what = format!("column `{}`", column.name);
            if let Some(name) = &expected.name
                && *name != column.name
            {
                self.mismatch(format!(
                    "column {} is `{}` in the database; the generated code reads `{name}`",
                    index + 1,
                    column.name
                ));
            }
            self.compare_type(&what, &column.type_name, &expected.rust_type, known);
            if column.nullable == Some(true) && !expected.nullable {
                self.mismatch(format!(
                    "{what} can be NULL in the database, but decodes into `{}`",
                    expected.rust_type
                ));
            }
        }
        let expected = expected_columns(&query.result, analysis).len();
        if !matches!(query.result, ResultShape::None) && expected != described.columns.len() {
            self.mismatch(format!(
                "answers {} column(s) in the database; the generated code reads {expected}",
                described.columns.len()
            ));
        }
    }

    fn compare_type(&mut self, what: &str, db_type: &str, rust_type: &str, known: &Known) {
        match known.accepts(rust_type, db_type) {
            Some(true) => {}
            Some(false) => self.mismatch(format!(
                "{what} is `{db_type}` in the database; the generated code uses `{rust_type}`"
            )),
            None => self.unchecked(format!("{what} (`{rust_type}`)")),
        }
    }
}

/// A result column as the generated code reads it.
struct Expected {
    /// The column name it reads by, when it reads by name.
    name: Option<String>,
    rust_type: String,
    nullable: bool,
}

fn expected_columns(result: &ResultShape, analysis: &Analysis) -> Vec<Expected> {
    match result {
        ResultShape::None => Vec::new(),
        ResultShape::Scalar(rust_type) => vec![Expected {
            name: None,
            nullable: types::is_option(rust_type),
            rust_type: rust_type.clone(),
        }],
        ResultShape::Row(columns) => columns
            .iter()
            .map(|column| Expected {
                name: Some(column.sql_name.clone()),
                nullable: types::is_option(&column.rust_type),
                rust_type: column.rust_type.clone(),
            })
            .collect(),
        ResultShape::Model { module, .. } => analysis
            .tables
            .iter()
            .find(|table| table.module_name == *module)
            .map(|table| {
                table
                    .columns
                    .iter()
                    .map(|column| Expected {
                        name: Some(column.sql_name.clone()),
                        rust_type: column.field_type(),
                        nullable: column.nullable,
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests;
