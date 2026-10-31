//! Where a monorepo keeps its contract and its Clients: the `[contract]`
//! and `[clients]` sections of its `rok.toml`.
//!
//! ```toml
//! [contract]
//! document = "apps/api/openapi.json"
//! export = ["cargo", "run", "--quiet", "-p", "axum-api", "--example", "openapi", "--"]
//!
//! [clients]
//! ts = "clients/ts"
//! ```
//!
//! `document` is the API's served document, written out; `export` is the
//! command that rewrites it, run from the root with the document's path
//! appended. Each `[clients]` entry names a client and its directory.
//! Paths are relative to the file's directory.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use super::openapi::Document;
use crate::report::display_path;

/// The clients rok emits, by their `[clients]` key.
pub const CLIENTS: &[&str] = &["ts"];

/// A monorepo's contract and Clients.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// The directory of the project file: what the paths are relative to,
    /// and where `export` runs.
    pub root: PathBuf,
    /// The contract: the API's `openapi.json`.
    pub document: PathBuf,
    /// The command that re-exports the document, before the document's
    /// path; none when the document is kept by other means.
    pub export: Option<Vec<String>>,
    /// The Clients, in the file's order.
    pub clients: Vec<Client>,
}

/// One Client: its kind (its `[clients]` key) and its directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    /// Its key, such as `ts`.
    pub name: String,
    /// Its directory.
    pub dir: PathBuf,
}

impl Project {
    /// The contract and Clients the project file at `path` names, or
    /// `None` when it names no `[clients]` (or does not exist).
    ///
    /// # Errors
    ///
    /// When the file is not TOML, names a client rok does not emit, or
    /// has `[clients]` without `[contract]`'s `document`.
    pub fn load(path: &Path) -> Result<Option<Project>> {
        if !path.is_file() {
            return Ok(None);
        }
        let shown = display_path(path);
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {shown}"))?;
        let table: toml::Table = toml::from_str(&text).with_context(|| shown.clone())?;
        let Some(clients) = table.get("clients") else {
            return Ok(None);
        };
        let root = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let clients = clients
            .as_table()
            .with_context(|| format!("{shown}: `clients` is not a table"))?
            .iter()
            .map(|(name, dir)| {
                if !CLIENTS.contains(&name.as_str()) {
                    bail!(
                        "{shown}: `clients.{name}`: rok emits the {} client(s) (workspace ADR 0027)",
                        CLIENTS.join(", ")
                    );
                }
                let dir = dir
                    .as_str()
                    .with_context(|| format!("{shown}: `clients.{name}` names no directory"))?;
                Ok(Client {
                    name: name.clone(),
                    dir: root.join(dir),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let contract = table.get("contract").and_then(toml::Value::as_table);
        let document = contract
            .and_then(|contract| contract.get("document"))
            .and_then(toml::Value::as_str)
            .with_context(|| {
                format!(
                    "{shown}: `[clients]` needs `[contract]`'s `document`, the API's openapi.json \
                     the clients are generated from"
                )
            })?;
        let export = match contract.and_then(|contract| contract.get("export")) {
            None => None,
            Some(value) => Some(
                value
                    .as_array()
                    .and_then(|parts| {
                        parts
                            .iter()
                            .map(|part| part.as_str().map(str::to_owned))
                            .collect::<Option<Vec<_>>>()
                    })
                    .filter(|parts| !parts.is_empty())
                    .with_context(|| {
                        format!("{shown}: `contract.export` is not a command (a list of strings)")
                    })?,
            ),
        };
        Ok(Some(Project {
            document: root.join(document),
            root,
            export,
            clients,
        }))
    }

    /// Re-exports the document: runs `export` from the root with the
    /// document's path appended. Its output goes to stderr, so a `--json`
    /// report on stdout stays one document. Does nothing without `export`.
    ///
    /// # Errors
    ///
    /// When the command does not start or fails.
    pub fn export(&self) -> Result<()> {
        let Some((program, args)) = self.export.as_ref().and_then(|parts| parts.split_first())
        else {
            return Ok(());
        };
        let document = std::path::absolute(&self.document)
            .with_context(|| format!("resolving {}", display_path(&self.document)))?;
        let shown = self
            .export
            .as_ref()
            .map(|parts| parts.join(" "))
            .unwrap_or_default();
        let status = Command::new(program)
            .args(args)
            .arg(&document)
            .current_dir(if self.root.as_os_str().is_empty() {
                Path::new(".")
            } else {
                &self.root
            })
            .stdin(Stdio::null())
            .stdout(Stdio::from(std::io::stderr()))
            .status()
            .with_context(|| format!("re-exporting the contract: `{shown}` did not start"))?;
        if !status.success() {
            bail!(
                "re-exporting the contract: `{shown}` failed ({status}); {} was not rewritten",
                display_path(&self.document)
            );
        }
        Ok(())
    }

    /// The contract, read.
    ///
    /// # Errors
    ///
    /// When the document cannot be read, or is outside the subset the
    /// clients read (the error names the construct and where).
    pub fn read_document(&self) -> Result<Document> {
        let shown = display_path(&self.document);
        let text = std::fs::read_to_string(&self.document)
            .with_context(|| format!("reading the contract {shown}"))?;
        Document::parse(&text).with_context(|| format!("reading the contract {shown}"))
    }
}
