//! Project config file loading and merging with command-line flags — the
//! same precedence rules the original `rok-db-gen` used: flags beat the
//! file, the file beats defaults.
//!
//! The canonical name is `rok.toml` (ADR 0011); the legacy
//! `rok-data.toml` stays readable as an alias and prints a note when
//! read.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use rok_data_codegen::{Dependency, Options, Rename};

/// The file contents. Unknown keys are errors, like before.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GenConfig {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    crate_name: Option<String>,
    rok_data_version: Option<String>,
    rok_data_path: Option<PathBuf>,
    strict: Option<bool>,
}

/// The full config file. Each section's schema belongs to the reader of
/// that section: the generator validates only `[generate]` (legacy
/// spelling `[gen]`); the other four sections exist here as file-level
/// schema names so a typo'd section is an error — their owning engine's
/// config reader applies them directly from the file.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigFile {
    #[serde(default)]
    #[allow(dead_code)]
    pub project: Option<toml::Table>,
    #[serde(default, rename = "gen", alias = "generate")]
    pub generate: Option<GenConfig>,
    #[serde(default)]
    #[allow(dead_code)]
    pub api: Option<toml::Table>,
    #[serde(default)]
    #[allow(dead_code)]
    pub conventions: Option<toml::Table>,
    #[serde(default)]
    #[allow(dead_code)]
    pub scaffold: Option<toml::Table>,
}

/// The canonical config file name.
pub(crate) const FILE: &str = "rok.toml";

/// The legacy config file name; readable forever, with a note when read.
pub(crate) const LEGACY_FILE: &str = "rok-data.toml";

/// Merge flags with an optional config file into generator options.
///
/// Paths in the file are relative to the file's directory; flag paths (and
/// defaults) are relative to the current directory. One parameter per flag,
/// as this is the single merge point for `rok generate`'s options.
#[allow(clippy::too_many_arguments)]
pub(crate) fn options(
    config_flag: Option<&Path>,
    input_flag: Option<PathBuf>,
    output_flag: Option<PathBuf>,
    crate_name: Option<&str>,
    migration: Option<String>,
    renames: Vec<Rename>,
    allow_destructive: bool,
    strict: bool,
    database_url: Option<String>,
) -> Result<Options, String> {
    let none = config_flag.is_none();
    let canonical_used = none && Path::new(FILE).is_file();
    let config_path = config_flag
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(if canonical_used { FILE } else { LEGACY_FILE }));
    let reading_legacy = config_path
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name == LEGACY_FILE);
    if none && canonical_used && Path::new(LEGACY_FILE).is_file() {
        eprintln!("note: ignoring `{LEGACY_FILE}`; `{FILE}` is the canonical name");
    }
    let (config, base) = match std::fs::read_to_string(&config_path) {
        Ok(text) => {
            if reading_legacy {
                eprintln!("note: `{LEGACY_FILE}` is a legacy name; move settings to `{FILE}`");
            }
            let file: ConfigFile =
                toml::from_str(&text).map_err(|e| format!("{}: {e}", config_path.display()))?;
            let base = config_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default();
            (file.generate.unwrap_or_default(), base)
        }
        Err(_) if none => (GenConfig::default(), PathBuf::new()),
        Err(e) => return Err(format!("{}: {e}", config_path.display())),
    };
    if config.rok_data_version.is_some() && config.rok_data_path.is_some() {
        return Err("set either rok_data_version or rok_data_path, not both".to_owned());
    }
    let mut options = Options::new();
    let from_config = |p: Option<PathBuf>, default: PathBuf| p.map_or(default, |p| base.join(p));
    options.input = input_flag.unwrap_or_else(|| from_config(config.input, options.input.clone()));
    options.output =
        output_flag.unwrap_or_else(|| from_config(config.output, options.output.clone()));
    if let Some(name) = crate_name.map(str::to_owned).or(config.crate_name) {
        options.crate_name = name;
    }
    if let Some(version) = config.rok_data_version {
        options.rok_data = Dependency::Version(version);
    }
    if let Some(path) = config.rok_data_path {
        options.rok_data = Dependency::Path(base.join(path));
    }
    options.strict = strict || config.strict.unwrap_or(false);
    options.database_url = database_url;
    options.migration = migration;
    options.renames = renames;
    options.allow_destructive = allow_destructive;
    Ok(options)
}
