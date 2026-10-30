//! Project config file loading and merging with command-line flags — the
//! same precedence rules the original `rok-db-gen` used: flags beat the
//! file, the file beats defaults.
//!
//! The canonical name is `rok.toml` (workspace ADR 0011); the legacy
//! `rok-data.toml` stays readable as an alias and prints a note when
//! read.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use dirs;
use rok_data_codegen::{Dependency, Options, Rename};
use serde::{Deserialize, Serialize};

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
/// spelling `[gen]`); the other five sections exist here as file-level
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
    /// `[tools.<tool>]`: read by rok-tools (`rok tools …`, ADR 0026).
    #[serde(default)]
    #[allow(dead_code)]
    pub tools: Option<toml::Table>,
}

/// The canonical config file name.
pub(crate) const FILE: &str = "rok.toml";

/// The legacy config file name; readable forever, with a note when read.
pub(crate) const LEGACY_FILE: &str = "rok-data.toml";

/// The keys the legacy generator's own `[gen]` section holds. Every other
/// key of a `[generate]` section belongs to `rok gen`.
const LEGACY_KEYS: &[&str] = &[
    "input",
    "output",
    "crate_name",
    "rok_data_version",
    "rok_data_path",
    "strict",
];

/// Reads the settings file: the legacy generator's own section, or an
/// error that says who owns the section it found.
fn parse(text: &str, path: &Path) -> Result<ConfigFile, String> {
    match toml::from_str::<ConfigFile>(text) {
        Ok(file) => Ok(file),
        Err(error) => Err(match unified_section(text, path) {
            Some(message) => message,
            None => format!("{}: {error}", path.display()),
        }),
    }
}

/// The message for the unified project's `[generate]` section, which the
/// legacy generator cannot use: it belongs to `rok gen`, and the legacy
/// generator reads its own `[gen]` section in the legacy file.
fn unified_section(text: &str, path: &Path) -> Option<String> {
    // The legacy file's own section is `[gen]`, in the legacy file; the
    // canonical file's `[generate]` section belongs to `rok gen` (ADR
    // 0011).
    if path.file_name().is_some_and(|name| name == LEGACY_FILE) {
        return None;
    }
    let table: toml::Table = toml::from_str(text).ok()?;
    let section = table.get("generate")?.as_table()?;
    let foreign = section
        .keys()
        .any(|key| !LEGACY_KEYS.contains(&key.as_str()));
    foreign.then(|| {
        format!(
            "{}: the `[generate]` section belongs to `rok gen`; the legacy generator reads \
             its own `[gen]` section in the legacy `{LEGACY_FILE}`",
            path.display()
        )
    })
}

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
            let file: ConfigFile = parse(&text, &config_path)?;
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

// ============================================================================
// Server config (servers.toml) — ADR 0028
// ============================================================================

/// A single registered PostgreSQL server.
///
/// The `database` field is the default database to connect to for
/// administrative operations (create/drop/query other databases).
/// It is not the project's database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    /// Unique name for this server (used as the keychain account).
    pub name: String,
    /// Hostname or IP address.
    pub host: String,
    /// Port number (default PostgreSQL: 5432).
    pub port: u16,
    /// Database user.
    pub user: String,
    /// Default database to connect to for admin operations (typically `postgres`).
    #[serde(default)]
    pub database: Option<String>,
    /// SSL mode: `disable`, `allow`, `prefer`, `require`, `verify-ca`, `verify-full`.
    #[serde(default)]
    pub sslmode: Option<String>,
}

/// The `servers.toml` file contents.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServersConfig {
    /// List of registered servers.
    #[serde(default)]
    pub servers: Vec<ServerConfig>,
}

/// Returns the per-user config directory for `rok`:
/// - Linux: `$XDG_CONFIG_HOME/rok` or `~/.config/rok`
/// - macOS: `~/Library/Application Support/rok`
/// - Windows: `%APPDATA%\rok`
///
/// Returns an error if the platform config directory cannot be determined
/// (e.g., on headless Linux/CI without HOME or XDG_CONFIG_HOME set).
pub fn config_dir() -> Result<PathBuf, String> {
    dirs::config_dir().map(|p| p.join("rok")).ok_or_else(|| {
        #[cfg(target_os = "linux")]
        {
            "cannot determine config directory: set HOME or XDG_CONFIG_HOME".to_owned()
        }
        #[cfg(target_os = "macos")]
        {
            "cannot determine config directory: set HOME".to_owned()
        }
        #[cfg(target_os = "windows")]
        {
            "cannot determine config directory: set APPDATA or USERPROFILE".to_owned()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            "cannot determine config directory on this platform".to_owned()
        }
    })
}

/// Returns the path to `servers.toml` in the config directory.
pub fn servers_path() -> Result<PathBuf, String> {
    config_dir().map(|p| p.join("servers.toml"))
}

/// Reads `servers.toml` from the given path (or the default location if `None`).
/// Returns an empty config if the file does not exist.
pub fn read_servers(path: Option<&Path>) -> Result<ServersConfig, String> {
    let path = path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| servers_path().unwrap());
    match fs::read_to_string(&path) {
        Ok(text) => {
            let config: ServersConfig =
                toml::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))?;
            Ok(config)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ServersConfig::default()),
        Err(e) => Err(format!("{}: {}", path.display(), e)),
    }
}

/// Writes `servers.toml` atomically: writes to a temp file, then renames.
/// Creates the config directory if it doesn't exist.
/// Sets file permissions to 0600 on Unix.
pub fn write_servers(path: &Path, config: &ServersConfig) -> Result<(), String> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }

    let text = toml::to_string_pretty(config).map_err(|e| e.to_string())?;

    // Atomic write: write to temp file, then rename
    let temp_path = path.with_extension("toml.tmp");
    fs::write(&temp_path, text).map_err(|e| format!("{}: {}", temp_path.display(), e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&temp_path)
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&temp_path, perms).map_err(|e| e.to_string())?;
    }
    #[cfg(windows)]
    {
        // On Windows, we rely on the default ACLs which are typically user-only
        let _ = fs::metadata(&temp_path);
    }

    fs::rename(&temp_path, path).map_err(|e| format!("{}: {}", path.display(), e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tools_section_belongs_to_rok_tools() {
        let path = Path::new(FILE);
        assert!(parse("[tools.loc]\nmax_lines = 600\n", path).is_ok());
        assert!(parse("[tool.loc]\nmax_lines = 600\n", path).is_err());
    }
}
