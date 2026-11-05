use std::fmt;
use std::path::{Path, PathBuf};

use crate::ApiEndpoint;

/// What a [`generate`](crate::generate) run did, or would do in a dry run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Report {
    /// Files written (in a dry run: files that would be written).
    pub written: Vec<PathBuf>,
    /// Scaffolded files written because they were absent (in a dry run:
    /// that would be written). An existing scaffolded file is the user's
    /// and is never listed, rewritten or removed.
    pub scaffolded: Vec<PathBuf>,
    /// Files whose contents were already up to date.
    pub unchanged: Vec<PathBuf>,
    /// Existing files rok-data-sqlc did not generate and would not overwrite.
    pub refused: Vec<RefusedFile>,
    /// Stale files: generated files that are no longer part of the plan.
    /// Removed after a successful run; a dry run or check only lists them.
    pub stale: Vec<PathBuf>,
    /// Check only: planned files that do not exist.
    pub missing: Vec<PathBuf>,
    /// Check only: existing planned files whose contents differ from the plan.
    pub changed: Vec<PathBuf>,
    /// Unsupported tables: skipped when lenient, fatal otherwise.
    pub table_failures: Vec<TableFailure>,
    /// Unsupported queries: skipped when lenient, fatal otherwise.
    pub query_failures: Vec<QueryFailure>,
    /// Problems the run found outside what it generates, such as a
    /// scaffolded file using what the configuration stopped generating.
    pub warnings: Vec<String>,
    /// API features served for the first time: the table's feature
    /// directory under `api.output` held no generated `mod.rs` before the
    /// run (in a dry run: features that would be). Empty in a check.
    pub newly_served: Vec<ServedFeature>,
}

impl Report {
    /// Whether the run found drift: missing, changed, or stale files.
    pub fn has_drift(&self) -> bool {
        !(self.missing.is_empty() && self.changed.is_empty() && self.stale.is_empty())
    }
}

/// An API feature a run serves for the first time ([`Report::newly_served`]).
/// Its [`Display`](fmt::Display) is the line a CLI prints: "now serving
/// `/api/v1/users` (list, create, get_one, update, remove); add `users` to
/// `[api] exclude` to keep it private".
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ServedFeature {
    /// The table's SQL name, schema-qualified outside the default schema.
    pub table: String,
    /// The route base the endpoints sit under, prefix included:
    /// `/api/v1/users`. `None` under gRPC, which serves no HTTP route.
    pub route: Option<String>,
    /// The endpoints served, in route order.
    pub endpoints: Vec<ApiEndpoint>,
    /// Whether `[api] include` names the table, so it is served on purpose.
    pub included: bool,
}

impl fmt::Display for ServedFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.route {
            Some(route) => write!(f, "now serving `{route}`")?,
            None => write!(f, "now serving `{}` over gRPC", self.table)?,
        }
        let endpoints: Vec<&str> = self.endpoints.iter().map(|e| e.name()).collect();
        write!(f, " ({})", endpoints.join(", "))?;
        if self.included {
            write!(f, "; `[api] include` names `{}`", self.table)
        } else {
            write!(
                f,
                "; add `{}` to `[api] exclude` to keep it private",
                self.table
            )
        }
    }
}

/// An existing file that generation would not overwrite.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RefusedFile {
    pub path: PathBuf,
    pub reason: String,
}

/// A table that cannot be generated.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TableFailure {
    /// The table's SQL name.
    pub table: String,
    pub reason: String,
    /// The schema file (migration) that defines the table.
    pub path: Option<PathBuf>,
    /// 1-based line of the table's `CREATE TABLE` in that file.
    pub line: Option<usize>,
}

impl TableFailure {
    /// `path:line: ` when the failure is located, else empty.
    pub(crate) fn location(&self) -> String {
        location(self.path.as_deref(), self.line)
    }
}

impl fmt::Display for TableFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}table `{}`: {}",
            self.location(),
            self.table,
            self.reason
        )
    }
}

/// A path as rok-data-sqlc prints it: relative to the current directory when it
/// lies under it, without a leading `./`, and with `/` as the separator on
/// every platform, so diagnostics and file lists read the same everywhere.
pub(crate) fn display_path(path: &Path) -> String {
    let current = std::env::current_dir().ok();
    let shown = current
        .as_deref()
        .and_then(|current| path.strip_prefix(current).ok())
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or(path);
    let mut text = shown.display().to_string();
    if std::path::MAIN_SEPARATOR == '\\' {
        text = text.replace('\\', "/");
    }
    while let Some(rest) = text.strip_prefix("./") {
        text = rest.to_owned();
    }
    text
}

/// The `path:line: ` prefix of a located diagnostic.
fn location(path: Option<&Path>, line: Option<usize>) -> String {
    match (path, line) {
        (Some(path), Some(line)) => format!("{}:{line}: ", display_path(path)),
        _ => String::new(),
    }
}

/// A query that cannot be generated.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct QueryFailure {
    /// The query name, or `line N` when no valid name was annotated.
    pub query: String,
    pub reason: String,
    /// The query file, when the failure is located in it.
    pub path: Option<PathBuf>,
    /// 1-based line of the query's `-- name:` annotation (or of the
    /// offending line when there is no valid annotation), when known.
    pub line: Option<usize>,
}

impl QueryFailure {
    /// `path:line: ` when the failure is located, else empty.
    pub(crate) fn location(&self) -> String {
        location(self.path.as_deref(), self.line)
    }
}

impl fmt::Display for QueryFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}query `{}`: {}",
            self.location(),
            self.query,
            self.reason
        )
    }
}

/// A failed [`generate`](crate::generate) run. Nothing was written.
///
/// [`Error::report`] holds what was known when the run stopped: the
/// unsupported tables and queries, or the refused files.
pub struct Error {
    inner: anyhow::Error,
    report: Box<Report>,
}

impl Error {
    pub(crate) fn new(inner: anyhow::Error, report: Report) -> Self {
        Self {
            inner,
            report: Box::new(report),
        }
    }

    /// What was known when the run stopped.
    pub fn report(&self) -> &Report {
        &self.report
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

/// The result of a [`generate`](crate::generate) run.
pub type Result<T, E = Error> = std::result::Result<T, E>;
