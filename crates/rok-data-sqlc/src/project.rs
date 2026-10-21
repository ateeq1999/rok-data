//! The project's settings: the config file the unified front door (`rok
//! gen`) reads.
//!
//! Two shapes are readable, and both name the same keys:
//!
//! - the unified project file `rok.toml` (engine ADR 0011), whose generation
//!   settings live in its `[generate]` section, beside the `[api]` and
//!   `[conventions]` sections;
//! - the engine's own file `rok-data-sqlc.toml`, whose keys sit at the top
//!   level.
//!
//! Paths in the file resolve against the file's directory; the flags beat
//! the file everywhere.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::Config;
use crate::config_file;

/// The config file names, in the order they are looked for: the canonical
/// `rok.toml` (engine ADR 0011) and the engine's legacy `rok-data-sqlc.toml`.
pub const FILE_NAMES: &[&str] = &["rok.toml", "rok-data-sqlc.toml"];

/// The nearest config file in `start` or one of its parents.
///
/// The canonical name wins over the legacy one in the same directory, and a
/// nearer directory wins over a farther one.
pub fn discover_config(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        for name in FILE_NAMES {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// The settings the config file at `path` holds.
///
/// The flags override them: the file says what the project wants, the
/// command line says what this run does.
pub fn load_config(path: &Path) -> Result<Config> {
    Ok(config_file::load_any(path)?.into_config())
}
