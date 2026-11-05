//! The runs a `rok gen` does, and each run's settings: one run per App a
//! Monorepo's project file names in `[apps]` at its root, the one run of
//! the App the command started inside, or the one run of a plain project,
//! its config file's values with the flags over them. Which it is, is the
//! one rule every command shares (`rok_data_sqlc::Project`).

use std::path::{Path, PathBuf};

use rok_data_codegen::Rename;
use rok_data_sqlc::{Config, Project, Target};

use super::{DEFAULT_SCHEMA_DIR, Run, Settings};
use crate::cli::r#gen::GenOptions;
use crate::config::FILE;

/// The project a run reads: `--config`'s, or the one the current directory
/// belongs to.
pub(super) fn project(options: &GenOptions) -> Result<Project, String> {
    match options.config.as_deref() {
        Some(path) => Project::at(path),
        None => Project::current(),
    }
    .map_err(|error| format!("{error:#}"))
}

/// The runs a `rok gen` does: one per App at a Monorepo's root, or the one
/// run of the App started inside or of a plain project.
pub(super) fn resolve(options: &GenOptions, check: bool) -> Result<Vec<Run>, String> {
    let project = project(options)?;
    let targets = project
        .targets(None)
        .map_err(|error| format!("{error:#}"))?;
    if matches!(project, Project::Monorepo { .. }) && !project.inside_app() {
        return resolve_apps(options, check, &targets);
    }
    targets
        .iter()
        .map(|target| {
            if target.app.is_some() && target.settings.is_none() {
                return Err(no_settings(target));
            }
            run(options, check, target, None)
        })
        .collect()
}

/// The run of one target; `app` names it in a run over a Monorepo's Apps.
fn run(
    options: &GenOptions,
    check: bool,
    target: &Target,
    app: Option<String>,
) -> Result<Run, String> {
    let settings = settings(options, check, target.settings.as_deref(), &target.dir)?;
    let using = target.using(&[
        ("schema", &settings.schema_dir),
        ("migrations", &settings.migrations_dir),
    ]);
    Ok(Run {
        app,
        settings,
        using,
    })
}

/// The error of an App with no settings file of its own.
fn no_settings(target: &Target) -> String {
    format!(
        "App `{}` (`{}`) has no settings file: the App's own settings are what a \
         regeneration reads, so add a `{FILE}` or the engine's `rok-data-sqlc.toml` there",
        target.app.as_deref().unwrap_or_default(),
        crate::output::display_path(&target.dir)
    )
}

/// The runs at a Monorepo's root: one per named App, each from the App's
/// own settings inside its directory.
fn resolve_apps(options: &GenOptions, check: bool, targets: &[Target]) -> Result<Vec<Run>, String> {
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
    targets
        .iter()
        .map(|target| {
            if target.settings.is_none() {
                return Err(no_settings(target));
            }
            run(options, check, target, target.app.clone())
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
    // A table or query the engine can't generate fails the run (the
    // file, the query and the fix are in its report); `--lenient` skips
    // it with a warning instead.
    engine.lenient = options.lenient;
    let renames = options
        .renames
        .iter()
        .map(|spec| {
            Rename::parse(spec)
                .ok_or_else(|| format!("--rename `{spec}`: expected `old=new` or `table.old=new`"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    // A path the file sets is already resolved against the file's
    // directory; the engine's bare default resolves against the base, so
    // an App's pairs land in the App's history dir.
    let migrations_dir = options.migrations_dir.clone().unwrap_or_else(|| {
        if engine.migrations == Config::default().migrations {
            join_default(base, &engine.migrations.to_string_lossy())
        } else {
            engine.migrations.clone()
        }
    });
    Ok(Settings {
        engine,
        schema_dir,
        migrations_dir,
        migration: options.migration.clone(),
        renames,
        allow_destructive: options.allow_destructive,
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
