//! `rok-data.toml` ([gen]) loading and merging with command-line flags — the
//! same precedence rules the original `rok-db-gen` used: flags beat the file,
//! the file beats defaults.

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

/// The full config file: one [gen] section.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigFile {
    #[serde(default, rename = "gen")]
    pub generate: GenConfig,
}

/// The constant name CLI users reach for.
pub(crate) const FILE: &str = "rok-data.toml";

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
    let config_path = config_flag
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(FILE));
    let (config, base) = match std::fs::read_to_string(&config_path) {
        Ok(text) => {
            let config: ConfigFile =
                toml::from_str(&text).map_err(|e| format!("{}: {e}", config_path.display()))?;
            let base = config_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default();
            (config.generate, base)
        }
        Err(_) if config_flag.is_none() => (GenConfig::default(), PathBuf::new()),
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
