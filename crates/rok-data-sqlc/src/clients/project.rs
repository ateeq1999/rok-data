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
//! dart = "clients/dart"
//! rust = "clients/rust"
//! ```
//!
//! `document` is the API's served document, written out; `export` is the
//! command that rewrites it, run from the root with the document's path
//! appended. Each `[clients]` entry names a client and its directory.
//! Paths are relative to the file's directory.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};

use super::openapi::{self, Document};
use crate::report::display_path;

/// A client rok emits. Adding one is a variant here: its `[clients]` key
/// and its emitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ClientKind {
    /// The TypeScript Client (`ts`).
    Ts,
    /// The Dart Client (`dart`).
    Dart,
    /// The Rust Client (`rust`).
    Rust,
}

impl ClientKind {
    /// Every client rok emits, in the order its errors list them.
    pub const ALL: [ClientKind; 3] = [ClientKind::Ts, ClientKind::Dart, ClientKind::Rust];

    /// Its `[clients]` key, such as `ts`.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            ClientKind::Ts => "ts",
            ClientKind::Dart => "dart",
            ClientKind::Rust => "rust",
        }
    }

    /// The client whose `[clients]` key is `key`.
    #[must_use]
    pub fn from_key(key: &str) -> Option<ClientKind> {
        ClientKind::ALL.into_iter().find(|kind| kind.key() == key)
    }

    /// Its files for `document`, each its path relative to its directory.
    ///
    /// # Errors
    ///
    /// When its emitter refuses the document.
    pub fn emit(self, document: &Document) -> Result<Vec<(PathBuf, String)>, super::emit::Error> {
        match self {
            ClientKind::Ts => super::ts::emit(document),
            ClientKind::Dart => super::dart::emit(document),
            ClientKind::Rust => super::rust::emit(document),
        }
    }
}

impl std::fmt::Display for ClientKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.key())
    }
}

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

/// One Client: its kind (named by its `[clients]` key) and its directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    /// Its kind.
    pub kind: ClientKind,
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
                let Some(kind) = ClientKind::from_key(name) else {
                    let keys: Vec<&str> = ClientKind::ALL.iter().map(|kind| kind.key()).collect();
                    bail!(
                        "{shown}: `clients.{name}`: rok emits the {} client(s) (workspace ADR 0027)",
                        keys.join(", ")
                    );
                };
                let dir = dir
                    .as_str()
                    .with_context(|| format!("{shown}: `clients.{name}` names no directory"))?;
                Ok(Client {
                    kind,
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
    /// clients read (the error names the construct and where). A `$ref` to
    /// an undefined schema also names the line that registers it.
    pub fn read_document(&self) -> Result<Document> {
        let shown = display_path(&self.document);
        let text = std::fs::read_to_string(&self.document)
            .with_context(|| format!("reading the contract {shown}"))?;
        Document::parse(&text)
            .map_err(|err| match &err {
                openapi::Error::UndefinedSchema { name, .. } => {
                    let path = self
                        .schema_path(name)
                        .unwrap_or_else(|| format!("<path to {name}>"));
                    anyhow!("{err}; register it in `openapi.rs`: `components(schemas({path}))`")
                }
                _ => anyhow::Error::new(err),
            })
            .with_context(|| format!("reading the contract {shown}"))
    }

    /// The path from the crate root of the type `name`, best-effort: the
    /// first `enum` or `struct` of that name in the API App's `src/` (the
    /// nearest `src/` from the document's directory up to the root).
    fn schema_path(&self, name: &str) -> Option<String> {
        let dir = self.document.parent()?;
        let src = dir
            .ancestors()
            .take_while(|dir| dir.starts_with(&self.root))
            .map(|dir| dir.join("src"))
            .find(|src| src.is_dir())?;
        let mut files = Vec::new();
        rust_files(&src, &mut files);
        files.sort();
        files
            .iter()
            .find(|file| defines(file, name))
            .and_then(|file| module_path(&src, file))
            .map(|module| format!("{module}::{name}"))
    }
}

/// Every `.rs` file under `dir`.
fn rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

/// Whether `file` declares `enum name` or `struct name`.
fn defines(file: &Path, name: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(file) else {
        return false;
    };
    text.lines().any(|line| {
        let line = line.trim_start();
        let line = line
            .strip_prefix("pub(crate) ")
            .or_else(|| line.strip_prefix("pub "))
            .unwrap_or(line);
        ["enum ", "struct "].iter().any(|item| {
            line.strip_prefix(item)
                .and_then(|rest| rest.strip_prefix(name))
                .is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
        })
    })
}

/// The module of `file` under `src`, from the crate root: `src/lib.rs` is
/// `crate`, `src/features/refunds/dto.rs` is `crate::features::refunds::dto`.
fn module_path(src: &Path, file: &Path) -> Option<String> {
    let relative = file.strip_prefix(src).ok()?.with_extension("");
    let mut parts: Vec<String> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.last().is_some_and(|last| last == "mod")
        || (parts.len() == 1 && (parts[0] == "lib" || parts[0] == "main"))
    {
        parts.pop();
    }
    parts.insert(0, "crate".to_owned());
    Some(parts.join("::"))
}

#[cfg(test)]
mod tests {
    use super::ClientKind;

    #[test]
    fn each_client_is_named_by_its_key() {
        for kind in ClientKind::ALL {
            assert_eq!(ClientKind::from_key(kind.key()), Some(kind));
            assert_eq!(kind.to_string(), kind.key());
        }
        assert_eq!(ClientKind::from_key("swift"), None);
    }
}
