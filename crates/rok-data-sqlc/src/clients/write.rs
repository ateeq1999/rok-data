//! Writing a Client into its directory, with the engine's generated-file
//! rules (workspace ADR 0005): a planned file is written when missing or
//! changed; an existing file without the marker (taken over by hand) is
//! refused unless forced; a marked file under the directory that is no
//! longer planned is stale and removed. A check writes nothing and
//! reports each of those as drift.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Result, bail};

use super::openapi::Document;
use super::project::{Client, Project};
use crate::output::{apply_files, compare_files, find_stale, remove_stale, write_absent};
use crate::report::display_path;

/// How a Client's files are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `rok gen`: bring the directory up to date.
    Write {
        /// Report what would change; write nothing.
        dry_run: bool,
        /// Overwrite files whose marker is gone.
        force: bool,
    },
    /// `rok gen check`: write nothing, report the drift.
    Check,
    /// `init`: write the files that do not exist, keep every other.
    Absent,
}

/// What writing (or checking) one Client did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The client's key, such as `ts`.
    pub client: String,
    /// Its directory.
    pub dir: PathBuf,
    /// Files written (in a dry run: that would be).
    pub written: Vec<PathBuf>,
    /// Files already up to date.
    pub unchanged: Vec<PathBuf>,
    /// Marked files no longer emitted: removed, or (a check, a dry run)
    /// that would be.
    pub stale: Vec<PathBuf>,
    /// A check's planned files that do not exist.
    pub missing: Vec<PathBuf>,
    /// A check's planned files whose contents differ.
    pub changed: Vec<PathBuf>,
}

impl Outcome {
    /// Whether a check found the Client out of date.
    pub fn has_drift(&self) -> bool {
        !self.missing.is_empty() || !self.changed.is_empty() || !self.stale.is_empty()
    }
}

/// Every Client of `project`, from its document, in `mode`.
///
/// # Errors
///
/// When the document cannot be read or emitted, a file cannot be
/// written, or a file without the marker would be overwritten.
pub fn generate(project: &Project, mode: Mode) -> Result<Vec<Outcome>> {
    let document = project.read_document()?;
    project
        .clients
        .iter()
        .map(|client| emit_client(&document, client, mode))
        .collect()
}

/// One Client's files under its directory, in `mode`.
///
/// # Errors
///
/// As [`generate`].
pub fn emit_client(document: &Document, client: &Client, mode: Mode) -> Result<Outcome> {
    let files: Vec<(PathBuf, String)> = match client.name.as_str() {
        "ts" => super::ts::emit(document)?,
        other => bail!("rok emits no `{other}` client"),
    }
    .into_iter()
    .map(|(path, text)| (client.dir.join(path), text))
    .collect();
    let mut outcome = Outcome {
        client: client.name.clone(),
        dir: client.dir.clone(),
        ..Outcome::default()
    };
    let keep: HashSet<PathBuf> = files.iter().map(|(path, _)| path.clone()).collect();
    match mode {
        Mode::Absent => {
            outcome.written = write_absent(&files)?;
            return Ok(outcome);
        }
        Mode::Check => {
            let (missing, changed) = compare_files(&files)?;
            outcome.unchanged = files
                .iter()
                .map(|(path, _)| path.clone())
                .filter(|path| !missing.contains(path) && !changed.contains(path))
                .collect();
            outcome.missing = missing;
            outcome.changed = changed;
            outcome.stale = find_stale(&client.dir, &keep)?;
        }
        Mode::Write { dry_run, force } => {
            let report = apply_files(&files, dry_run, force)?;
            if !report.refused.is_empty() {
                let refused: Vec<String> = report
                    .refused
                    .iter()
                    .map(|(path, reason)| format!("{}: {reason}", display_path(path)))
                    .collect();
                bail!(
                    "the `{}` client was not written:\n  {}",
                    client.name,
                    refused.join("\n  ")
                );
            }
            outcome.written = report.written;
            outcome.unchanged = report.unchanged;
            outcome.stale = find_stale(&client.dir, &keep)?;
            if !dry_run {
                remove_stale(&client.dir, &outcome.stale)?;
            }
        }
    }
    Ok(outcome)
}
