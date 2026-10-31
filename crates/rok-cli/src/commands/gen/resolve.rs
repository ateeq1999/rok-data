//! The runs a `rok gen` does, and each run's settings: one run per App a
//! monorepo's project file names in `[apps]`, or the one run of a plain
//! project, its config file's values with the flags over them.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use rok_data_codegen::Rename;
use rok_data_sqlc::Config;

use super::{DEFAULT_SCHEMA_DIR, Run, Settings};
use crate::cli::r#gen::GenOptions;
use crate::config::FILE;
use crate::output::display_path;

/// The runs a `rok gen` does: one per App a project file named, or the
/// one run of a plain project.
pub(super) fn resolve(options: &GenOptions, check: bool) -> Result<Vec<Run>, String> {
    let file = match options.config.as_deref() {
        Some(path) => Some(path.to_path_buf()),
        None => std::env::current_dir()
            .ok()
            .and_then(|dir| rok_data_sqlc::discover_config(&dir)),
    };
    if let Some(path) = file.as_deref()
        && let Some(apps) = apps_section(path)?
    {
        return resolve_apps(options, check, path, &apps);
    }
    let settings = settings(options, check, file.as_deref(), Path::new(""))?;
    Ok(vec![Run {
        app: None,
        settings,
    }])
}

/// The `[apps]` section of a canonical project file: the Apps of a
/// monorepo (workspace ADR 0027), each a name and the directory its
/// settings live in, relative to the file's. `None` when the file is
/// another name or names no such section.
fn apps_section(path: &Path) -> Result<Option<Vec<(String, String)>>, String> {
    if path.file_name().and_then(OsStr::to_str) != Some(FILE) {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("{}: {error}", display_path(path)))?;
    let table: toml::Table =
        toml::from_str(&text).map_err(|error| format!("{}: {error}", display_path(path)))?;
    let Some(section) = table.get("apps").and_then(|apps| apps.as_table()) else {
        return Ok(None);
    };
    let apps = section
        .iter()
        .map(|(name, dir)| {
            let dir = dir.as_str().ok_or_else(|| {
                format!(
                    "{}: the `apps.{name}` entry names no directory",
                    display_path(path)
                )
            })?;
            Ok((name.clone(), dir.to_owned()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Some(apps))
}

/// The runs of a monorepo project: one per named App, each from the App's
/// own settings inside its directory.
fn resolve_apps(
    options: &GenOptions,
    check: bool,
    file: &Path,
    apps: &[(String, String)],
) -> Result<Vec<Run>, String> {
    // One App's own flags don't compose with a run over every named App:
    // the App's own settings carry them.
    let mut per_app: Vec<&str> = Vec::new();
    if options.schema.is_some() {
        per_app.push("--schema");
    }
    if options.output.is_some() {
        per_app.push("--output");
    }
    if !options.queries.is_empty() {
        per_app.push("--queries");
    }
    if options.resource.is_some() {
        per_app.push("--resource");
    }
    if options.migrations_dir.is_some() {
        per_app.push("--migrations-dir");
    }
    if options.api_framework.is_some() {
        per_app.push("--api-framework");
    }
    if !per_app.is_empty() {
        return Err(format!(
            "in a project whose `[apps]` names the Apps, {} shape one App: \
             pass them from that App's directory, where its own settings apply",
            per_app.join(", ")
        ));
    }
    let base = file.parent().map(Path::to_path_buf).unwrap_or_default();
    apps.iter()
        .map(|(name, dir)| {
            let app = base.join(dir);
            let config = rok_data_sqlc::discover_config(&app).ok_or_else(|| {
                format!(
                    "App `{name}` (`{dir}`) has no settings file: the App's own settings \
                     are what a regeneration reads, so add a `{FILE}` or the engine's \
                     `rok-data-sqlc.toml` there"
                )
            })?;
            let settings = settings(options, check, Some(&config), &app)?;
            Ok(Run {
                app: Some(name.clone()),
                settings,
            })
        })
        .collect()
}

/// The run's settings: the config file's values with the flags over
/// them. Paths in the file resolve against the file's directory; the
/// flags beat the file everywhere (workspace ADR 0011). When no config
/// file holds the defaults — a plain project without one — they resolve
/// against the current directory, so `base` is empty for a plain
/// project's run and the App's directory for an App's run.
fn settings(
    options: &GenOptions,
    check: bool,
    file: Option<&Path>,
    base: &Path,
) -> Result<Settings, String> {
    let mut engine = match file {
        Some(path) => rok_data_sqlc::load_config(path).map_err(|e| format!("{e:#}"))?,
        None => Config::default(),
    };
    // The flags beat the file (workspace ADR 0011).
    if let Some(framework) = options.api_framework {
        let api = engine.api.as_mut().ok_or_else(|| {
            "`--api-framework` needs the `[api]` section of the settings file \
             (with its `output`): the run generates no API features"
                .to_owned()
        })?;
        api.framework = framework.into();
    }
    let entries: Vec<PathBuf> = match options.schema.as_ref() {
        Some(dir) => vec![dir.clone()],
        None if engine.schema.is_empty() => vec![join_default(base, DEFAULT_SCHEMA_DIR)],
        None => engine.schema.clone(),
    };
    let schema_dir = match options.schema.as_deref() {
        Some(dir) => dir.to_path_buf(),
        None => entries
            .iter()
            .find(|entry| entry.is_dir())
            .cloned()
            .or_else(|| entries.first().cloned())
            .unwrap_or_else(|| join_default(base, DEFAULT_SCHEMA_DIR)),
    };
    engine.schema = schema_inputs(&entries)?;
    if let Some(output) = options.output.clone() {
        engine.output = output;
    }
    if let Some(resource) = options.resource.clone() {
        engine.resource = Some(resource);
    }
    if !options.queries.is_empty() {
        engine.queries = options.queries.clone();
    }
    engine.dry_run = options.dry_run;
    engine.force = options.force;
    engine.check = check;
    // The unsupported tables and queries are reported as warnings; the
    // `--strict` flag turns them into failures.
    engine.lenient = true;
    let renames = options
        .renames
        .iter()
        .map(|spec| {
            Rename::parse(spec)
                .ok_or_else(|| format!("--rename `{spec}`: expected `old=new` or `table.old=new`"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let migrations_dir = options
        .migrations_dir
        .clone()
        .unwrap_or_else(|| engine.migrations.clone());
    Ok(Settings {
        engine,
        schema_dir,
        migrations_dir,
        migration: options.migration.clone(),
        renames,
        allow_destructive: options.allow_destructive,
        strict: options.strict,
        json: options.json,
    })
}

/// The default schema dir relative to the run's base: the config file's
/// directory, or the current directory for a plain project without one.
fn join_default(base: &Path, dir: &str) -> PathBuf {
    if base.as_os_str().is_empty() {
        PathBuf::from(dir)
    } else {
        base.join(dir)
    }
}

/// The schema inputs of the schema entries: a file as it is, a directory as
/// its truth files (the plain `*.sql` files), without the staged migration
/// pairs — those are the history, and reading them would apply every
/// change twice. A directory that holds only staged migrations is the
/// truth: the engine reads it and skips the `.down.sql` files.
fn schema_inputs(entries: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut inputs = Vec::new();
    for entry in entries {
        if !entry.is_dir() {
            inputs.push(entry.clone());
            continue;
        }
        let truth = rok_data_codegen::truth_files(entry).map_err(|e| e.to_string())?;
        if truth.is_empty() {
            inputs.push(entry.clone());
        } else {
            inputs.extend(truth);
        }
    }
    Ok(inputs)
}
