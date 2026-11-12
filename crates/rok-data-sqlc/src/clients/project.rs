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
//! dart = { dir = "apps/mobile/lib/app/data/api", layout = "sources" }
//! rust = "clients/rust"
//! ```
//!
//! `document` is the API's served document, written out; `export` is the
//! command that rewrites it, run from the root with the document's path
//! appended. Each `[clients]` entry names a client and its directory: a
//! string is the directory of a package, a table gives the directory and
//! the [`Layout`] (`package`, or `sources` for the TypeScript and Dart
//! Clients). Paths are relative to the file's directory.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};

use super::layout::Layout;
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

/// One Client: its kind (named by its `[clients]` key), its directory and
/// how its files sit there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    /// Its kind.
    pub kind: ClientKind,
    /// Its directory.
    pub dir: PathBuf,
    /// How its files sit in its directory.
    pub layout: Layout,
}

impl Client {
    /// Its files for `document`, laid out, each its path relative to its
    /// directory.
    ///
    /// # Errors
    ///
    /// When its emitter refuses the document.
    pub fn files(&self, document: &Document) -> Result<Vec<(PathBuf, String)>, super::emit::Error> {
        Ok(self.layout.apply(self.kind, self.kind.emit(document)?))
    }
}

/// The `[clients]` entry `name`'s directory and layout: a string is a
/// package's directory; a table is `{ dir = "…", layout = "…" }`.
fn entry(
    shown: &str,
    name: &str,
    kind: ClientKind,
    value: &toml::Value,
) -> Result<(String, Layout)> {
    if let Some(dir) = value.as_str() {
        return Ok((dir.to_owned(), Layout::Package));
    }
    let Some(table) = value.as_table() else {
        bail!("{shown}: `clients.{name}` names no directory");
    };
    if let Some(key) = table
        .keys()
        .find(|key| !["dir", "layout"].contains(&key.as_str()))
    {
        bail!("{shown}: `clients.{name}.{key}`: a client takes `dir` and `layout`");
    }
    let dir = table
        .get("dir")
        .and_then(toml::Value::as_str)
        .with_context(|| format!("{shown}: `clients.{name}` names no `dir`"))?;
    let layout = match table.get("layout") {
        None => Layout::Package,
        Some(value) => {
            let key = value.as_str().unwrap_or_default();
            Layout::from_key(key).with_context(|| {
                let layouts: Vec<&str> = Layout::ALL.iter().map(|layout| layout.key()).collect();
                format!(
                    "{shown}: `clients.{name}.layout`: {value} is not a layout ({})",
                    layouts.join(", ")
                )
            })?
        }
    };
    if !layout.supports(kind) {
        bail!(
            "{shown}: `clients.{name}.layout`: the {kind} Client is a package only; \
             `layout = \"{layout}\"` is for the ts and dart Clients"
        );
    }
    Ok((dir.to_owned(), layout))
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
            .map(|(name, value)| {
                let Some(kind) = ClientKind::from_key(name) else {
                    let keys: Vec<&str> = ClientKind::ALL.iter().map(|kind| kind.key()).collect();
                    bail!(
                        "{shown}: `clients.{name}`: rok emits the {} client(s) (workspace ADR 0027)",
                        keys.join(", ")
                    );
                };
                let (dir, layout) = entry(&shown, name, kind, value)?;
                Ok(Client {
                    kind,
                    dir: root.join(dir),
                    layout,
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
    /// When the command does not start or fails. A `cargo` export that
    /// fails because the App doesn't compile says so, with the compiler's
    /// first error.
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
        let cargo = Path::new(program)
            .file_stem()
            .is_some_and(|stem| stem.eq_ignore_ascii_case("cargo"));
        let mut child = Command::new(program)
            .args(args)
            .arg(&document)
            .current_dir(if self.root.as_os_str().is_empty() {
                Path::new(".")
            } else {
                &self.root
            })
            .stdin(Stdio::null())
            .stdout(Stdio::from(std::io::stderr()))
            // A `cargo` export's diagnostics are read (and still shown as
            // they come) to tell an App that doesn't build from an export
            // that fails.
            .stderr(if cargo {
                Stdio::piped()
            } else {
                Stdio::inherit()
            })
            .spawn()
            .with_context(|| format!("re-exporting the contract: `{shown}` did not start"))?;
        let diagnostics = child.stderr.take().map(tee).unwrap_or_default();
        let status = child
            .wait()
            .with_context(|| format!("re-exporting the contract: `{shown}` did not finish"))?;
        if !status.success() {
            if let Some(error) = first_compile_error(&diagnostics) {
                bail!(
                    "the App doesn't build after this change: fix the call sites, then run \
                     `rok gen` to write the contract and the Clients\n\n{error}"
                );
            }
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

/// Copies `stream` to stderr as it comes, line by line, and returns what
/// it held.
fn tee(stream: impl Read) -> String {
    let mut reader = BufReader::new(stream);
    let mut held = Vec::new();
    let mut line = Vec::new();
    while reader
        .read_until(b'\n', &mut line)
        .is_ok_and(|read| read > 0)
    {
        // Best effort: a closed stderr must not stop the export.
        let _ = std::io::stderr().write_all(&line);
        held.append(&mut line);
    }
    String::from_utf8_lossy(&held).into_owned()
}

/// The most lines of the compiler's first error a summary shows.
const ERROR_LINES: usize = 12;

/// The compiler's first error in cargo's `diagnostics`, when cargo could
/// not compile a crate: its `error` line and the code it points at, up to
/// its first note or help (or the blank line that ends it).
fn first_compile_error(diagnostics: &str) -> Option<String> {
    let diagnostics = &without_colours(diagnostics);
    let cargo_line = |line: &str| {
        line.starts_with("error: could not compile") || line.starts_with("error: aborting")
    };
    if !diagnostics
        .lines()
        .any(|line| line.starts_with("error: could not compile"))
    {
        return None;
    }
    let lines: Vec<&str> = diagnostics
        .lines()
        .skip_while(|line| !line.starts_with("error") || cargo_line(line))
        .enumerate()
        .take_while(|(at, line)| {
            *at == 0
                || !(line.trim().is_empty()
                    || line.starts_with("note:")
                    || line.starts_with("help:"))
        })
        .map(|(_, line)| line)
        .take(ERROR_LINES)
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// `text` without its terminal colour codes (`ESC [ … m` and the other
/// `ESC [` sequences cargo writes when told to colour).
fn without_colours(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            // Parameters and intermediates, then one final byte in `@..=~`.
            for c in chars.by_ref() {
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
        } else {
            plain.push(c);
        }
    }
    plain
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

    /// Cargo colours its diagnostics when `CARGO_TERM_COLOR=always` (as CI
    /// sets it); the summary still finds the error, without the escapes.
    #[test]
    fn a_coloured_compile_error_is_still_found() {
        let diagnostics = "\u{1b}[1m\u{1b}[91merror[E0061]\u{1b}[0m\u{1b}[1m: this function takes 2 arguments but 1 argument was supplied\u{1b}[0m\n \
             \u{1b}[1m\u{1b}[94m--> \u{1b}[0msrc/main.rs:6:20\n\n\
             \u{1b}[1m\u{1b}[96mhelp\u{1b}[0m: provide the argument\n\
             \u{1b}[1m\u{1b}[91merror\u{1b}[0m: could not compile `exporter` (bin \"exporter\") due to 1 previous error\n";
        let error = super::first_compile_error(diagnostics).expect("the error is found");
        assert_eq!(
            error,
            "error[E0061]: this function takes 2 arguments but 1 argument was supplied\n --> src/main.rs:6:20"
        );
    }
}
