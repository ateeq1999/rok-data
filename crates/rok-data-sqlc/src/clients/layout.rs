//! How a Client's files sit in its directory: as a package of its
//! language (the default), or as plain sources an App keeps among its own
//! (`layout = "sources"`, workspace ADR 0027 as amended 2026-11-03).
//!
//! A `sources` Client is the package's source files, flat: the
//! TypeScript Client's `index.ts`, `runtime.ts`, `types.ts`, `calls.ts`
//! and `queries.ts`; the Dart Client's `api.dart` (the export file),
//! `runtime.dart`, `models.dart` and `client.dart`. No README, manifest or
//! `src/` level. The files import each other by relative path, so they
//! read the same in either layout. The Rust Client is a crate, and has
//! only the package layout.

use std::path::{Path, PathBuf};

use super::project::ClientKind;

/// How a Client's files sit in its directory.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Layout {
    /// A package of the Client's language: its manifest, README and
    /// sources (`clients.ts = "clients/ts"`).
    #[default]
    Package,
    /// The source files only, flat in the directory, for an App that keeps
    /// the Client among its own sources
    /// (`clients.ts = { dir = "apps/web/src/lib/api", layout = "sources" }`).
    Sources,
}

impl Layout {
    /// Every layout, in the order errors list them.
    pub const ALL: [Layout; 2] = [Layout::Package, Layout::Sources];

    /// Its name in `[clients]`, such as `sources`.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Layout::Package => "package",
            Layout::Sources => "sources",
        }
    }

    /// The layout named `key`.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Layout> {
        Layout::ALL.into_iter().find(|layout| layout.key() == key)
    }

    /// Whether the `kind` Client can be laid out this way.
    #[must_use]
    pub fn supports(self, kind: ClientKind) -> bool {
        self == Layout::Package || kind != ClientKind::Rust
    }

    /// `files`, the `kind` Client's package, laid out this way.
    #[must_use]
    pub fn apply(self, kind: ClientKind, files: Vec<(PathBuf, String)>) -> Vec<(PathBuf, String)> {
        if self == Layout::Package {
            return files;
        }
        let mut laid: Vec<(PathBuf, String)> = files
            .into_iter()
            .filter_map(|(path, text)| source(kind, &path, text))
            .collect();
        laid.sort_by(|a, b| a.0.cmp(&b.0));
        laid
    }
}

impl std::fmt::Display for Layout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.key())
    }
}

/// Where a package file of the `kind` Client goes among plain sources, and
/// its text there; `None` for a file only a package has.
fn source(kind: ClientKind, path: &Path, text: String) -> Option<(PathBuf, String)> {
    match kind {
        ClientKind::Ts => {
            let file = path.strip_prefix("src").ok()?;
            Some((file.to_path_buf(), text))
        }
        ClientKind::Dart => {
            if path == Path::new("lib/api_client.dart") {
                return Some((
                    PathBuf::from("api.dart"),
                    text.replace("export 'src/", "export '"),
                ));
            }
            let file = path.strip_prefix("lib/src").ok()?;
            Some((file.to_path_buf(), text))
        }
        ClientKind::Rust => Some((path.to_path_buf(), text)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_layout_is_named_by_its_key() {
        for layout in Layout::ALL {
            assert_eq!(Layout::from_key(layout.key()), Some(layout));
        }
        assert_eq!(Layout::from_key("flat"), None);
        assert_eq!(Layout::default(), Layout::Package);
    }

    #[test]
    fn the_rust_client_has_only_the_package_layout() {
        assert!(Layout::Package.supports(ClientKind::Rust));
        assert!(!Layout::Sources.supports(ClientKind::Rust));
        assert!(Layout::Sources.supports(ClientKind::Ts));
        assert!(Layout::Sources.supports(ClientKind::Dart));
    }
}
