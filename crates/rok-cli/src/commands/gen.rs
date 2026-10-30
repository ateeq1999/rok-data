//! `rok gen` / `rok gen check`: the unified front door.
//!
//! Two things, both over the project's SQL:
//!
//! 1. the engine's generator (`rok_data_sqlc`): the data layer in
//!    `src/db/`, the shared tools, and the API features in `src/features/`,
//!    built from the **truth dir** (its plain `*.sql` files, `--schema`);
//! 2. the codegen crate's migration staging (`--migration NAME`): the
//!    difference between the truth and the state of the last staged
//!    migration, written as a `NNNN_name.up.sql` pair into the **history
//!    dir** (`--migrations-dir`). The snapshot the next staging diffs
//!    against lives there too, hidden beside the pairs.
//!
//! The truth dir is the project's only source of truth (workspace ADR
//! 0010); the history dir is what `rok migrate` reads. The two default
//! to one — `migrations`, the layout `rok init` seeded before the split
//! — so a project of that layout keeps generating and staging as before.
//!
//! A project file whose
//!
//! ```toml
//! [apps]
//! api = "apps/api"
//! ```
//!
//! names the Apps of a monorepo (workspace ADR 0027): the run then
//! regenerates every named App from its own settings file inside it, and
//! stages each App's migrations into its own history dir. The flags that
//! shape one project (`--schema`, `--output`, `--queries`, `--resource`,
//! `--migrations-dir`, `--api-framework`) are refused: they belong to one
//! App, saying them in that App's directory does.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rok_data_codegen::{Rename, StageOptions, Staged};
use rok_data_sqlc::{Config, Report};
use serde_json::Value;

use crate::cli::{GenAction, GenOptions};
use crate::config::FILE;
use crate::output::display_path;

mod report;

use report::{print_json_body, print_report};

/// The truth dir a project has when nothing else is said: also where the
/// pre-split layout (`rok init` before the history was separated) keeps
/// its staged pairs.
const DEFAULT_SCHEMA_DIR: &str = "migrations";

/// The history dir the settings place the staged pairs in when nothing
/// moves them: the same dir `rok migrate` reads by default.
const DEFAULT_MIGRATIONS_DIR: &str = "migrations";

/// `rok gen`: with the flags directly, or with the nested `rok gen check`
/// CI mode. A value given at the group level and again on the leaf: the
/// leaf wins; booleans OR.
pub(crate) fn run(options: GenOptions, action: Option<GenAction>) -> Result<ExitCode, String> {
    let Some(GenAction::Check { options: leaf }) = action else {
        return generate(options);
    };
    if options.migration.is_some() || !options.renames.is_empty() {
        // The flags belong to the generate run; don't silently ignore them
        // now that `check` nests under `gen`.
        return Err(
            "`--migration` and `--rename` belong to the generate run, not `check`: \
             pass them to `rok gen` without `check`"
                .to_owned(),
        );
    }
    check(leaf.over(options))
}

/// One run of the generator: an App the project file named in `[apps]`,
/// or the run of a plain project.
struct Run {
    /// The App this run regenerates (`api`), or `None` for a plain
    /// project's run.
    app: Option<String>,
    /// The run's settings.
    settings: Settings,
}

/// `rok gen`: the data layer and the API features, and the migration pair
/// each run stages when `--migration NAME` is given.
fn generate(options: GenOptions) -> Result<ExitCode, String> {
    let runs = resolve(&options, false)?;
    let multi = runs.iter().any(|run| run.app.is_some());
    let mut bodies: Vec<Value> = Vec::new();
    for run in &runs {
        heading(run, multi);
        match rok_data_sqlc::generate(&run.settings.engine) {
            Ok(report) => {
                let strict = strict_failure(&run.settings, &report);
                let staged = if strict.is_none() {
                    stage(&run.settings)?
                } else {
                    None
                };
                if run.settings.json {
                    bodies.push(print_json_body(
                        &run.settings,
                        run.app.as_deref(),
                        &report,
                        staged.as_ref(),
                        None,
                        false,
                    ));
                } else {
                    print_report(&run.settings, &report, staged.as_ref(), None);
                    next_steps(&run.settings, staged.as_ref());
                }
                if let Some(message) = strict {
                    return print_bodies_then(bodies, multi, Err(message));
                }
            }
            Err(error) => {
                let report = error.report();
                if run.settings.json {
                    bodies.push(print_json_body(
                        &run.settings,
                        run.app.as_deref(),
                        report,
                        None,
                        None,
                        false,
                    ));
                } else {
                    print_report(&run.settings, report, None, None);
                }
                return print_bodies_then(bodies, multi, Err(error.to_string()));
            }
        }
    }
    print_bodies_then(bodies, multi, Ok(ExitCode::SUCCESS))
}

/// `rok gen check`: the CI mode. Nothing is written; each run's drift
/// (missing, changed, or stale files) and its un-staged truth change
/// ([`staged_drift`]) are reported, and either fails the run.
fn check(options: GenOptions) -> Result<ExitCode, String> {
    let runs = resolve(&options, true)?;
    let multi = runs.iter().any(|run| run.app.is_some());
    let mut bodies: Vec<Value> = Vec::new();
    let mut findings: Vec<String> = Vec::new();
    for run in &runs {
        heading(run, multi);
        let report = match rok_data_sqlc::generate(&run.settings.engine) {
            Ok(report) => report,
            Err(error) => {
                let report = error.report();
                let drift = report.has_drift();
                if run.settings.json {
                    bodies.push(print_json_body(
                        &run.settings,
                        run.app.as_deref(),
                        report,
                        None,
                        None,
                        drift,
                    ));
                } else {
                    print_report(&run.settings, report, None, None);
                }
                if !drift {
                    // Not a drift: the run failed for another reason.
                    return print_bodies_then(bodies, multi, Err(error.to_string()));
                }
                findings.push(out_of_date_message(run));
                continue;
            }
        };
        let strict = strict_failure(&run.settings, &report);
        let pending = if strict.is_none() {
            staged_drift(&run.settings)?
        } else {
            None
        };
        if run.settings.json {
            bodies.push(print_json_body(
                &run.settings,
                run.app.as_deref(),
                &report,
                None,
                pending.as_ref(),
                false,
            ));
        } else {
            print_report(&run.settings, &report, None, pending.as_ref());
        }
        if let Some(message) = strict {
            return print_bodies_then(bodies, multi, Err(message));
        }
        if pending.is_some() {
            findings.push(pending_message(run));
        }
    }
    print_bodies(bodies, multi);
    if findings.is_empty() {
        // A plain project's prose says it as it ends; the JSON object
        // carries the same in its `drift` field, as before.
        if !options.json {
            println!("the generated code is up to date");
        }
        Ok(ExitCode::SUCCESS)
    } else {
        for finding in &findings {
            eprintln!("error: {finding}");
        }
        Ok(ExitCode::FAILURE)
    }
}

/// The finding a check run makes of an App whose generated code drifted.
fn out_of_date_message(run: &Run) -> String {
    match &run.app {
        Some(app) => {
            format!("{app}'s generated code is out of date; run `rok gen` to update it")
        }
        None => "the generated code is out of date; run `rok gen` to update it".to_owned(),
    }
}

/// An un-staged truth change the check reports: the pair a
/// `rok gen --migration <name>` would stage next. The check cannot know
/// the user's migration name, so the finding carries the placeholder the
/// planned staging names its would-be pair — `pending` — instead of a
/// fabricated path in the report's `missing` files.
struct Pending {
    /// The last staged migration's stem (`0001_init`), the shape the
    /// truth moved on from.
    base: String,
    /// The version the pair takes when it is staged: the next free one
    /// in the history dir.
    version: i64,
    /// The pair's placeholder name in the finding.
    name: String,
}

/// The finding a check run makes of a truth move nothing has staged: the
/// pair a `--migration <name>` run would stage next, which the run plans
/// as `pending`.
fn pending_message(run: &Run) -> String {
    match &run.app {
        Some(app) => format!(
            "the truth of `{app}` has a change that is not staged as a migration; \
             run `rok gen --migration NAME` in the app's directory"
        ),
        None => "the truth has a change that is not staged as a migration; \
                 run `rok gen --migration NAME` in the repo"
            .to_owned(),
    }
}

/// Prints the `--json` bodies the runs collected — a plain project's one
/// object, a named-Apps project's array of objects. Prose runs collected
/// none, and print nothing here.
fn print_bodies(bodies: Vec<Value>, multi: bool) {
    if !bodies.is_empty() {
        if multi {
            println!("{}", Value::Array(bodies));
        } else {
            for body in bodies {
                println!("{body}");
            }
        }
    }
}

/// [`print_bodies`], then the outcome.
fn print_bodies_then(
    bodies: Vec<Value>,
    multi: bool,
    outcome: Result<ExitCode, String>,
) -> Result<ExitCode, String> {
    print_bodies(bodies, multi);
    outcome
}

/// Names the run before its findings, when the project has named Apps.
fn heading(run: &Run, multi: bool) {
    if multi && let Some(app) = &run.app {
        println!("{app}:");
    }
}

/// The message of a `--strict` run that found warnings, if any.
fn strict_failure(settings: &Settings, report: &Report) -> Option<String> {
    if !settings.strict {
        return None;
    }
    let mut reasons: Vec<String> = Vec::new();
    for failure in &report.table_failures {
        reasons.push(format!("table `{}`: {}", failure.table, failure.reason));
    }
    for failure in &report.query_failures {
        reasons.push(format!("query `{}`: {}", failure.query, failure.reason));
    }
    for warning in &report.warnings {
        reasons.push(warning.clone());
    }
    if reasons.is_empty() {
        return None;
    }
    Some(format!(
        "--strict: {} warning(s):\n  - {}",
        reasons.len(),
        reasons.join("\n  - ")
    ))
}

/// The migration pair the run stages, if any.
fn stage(settings: &Settings) -> Result<Option<Staged>, String> {
    let Some(name) = &settings.migration else {
        return Ok(None);
    };
    if !settings.schema_dir.is_dir() {
        return Err(format!(
            "--migration needs a schema directory: `{}` is not one",
            display_path(&settings.schema_dir)
        ));
    }
    let staged = rok_data_codegen::stage(&settings.stage_options(name, settings.engine.dry_run))
        .map_err(|e| e.to_string())?;
    Ok(Some(staged))
}

/// The run's settings: the config file's values with the flags over them.
struct Settings {
    /// The engine's configuration.
    engine: Config,
    /// The truth dir the generator reads and the staging diffs.
    schema_dir: PathBuf,
    /// The history dir the staged pairs are written into.
    migrations_dir: PathBuf,
    /// The migration to stage, when the run stages one.
    migration: Option<String>,
    /// The renames to record instead of a drop-and-add.
    renames: Vec<Rename>,
    /// Whether the steps that lose data are allowed.
    allow_destructive: bool,
    /// Whether a warning fails the run.
    strict: bool,
    /// Whether the report is JSON.
    json: bool,
}

impl Settings {
    /// The staging options both the run's real staging (`--migration`) and
    /// the check's planned pair build from: the same truth and history
    /// dirs, the same renames and guard-rails — so the check's planned
    /// pair models the run it is checking. The name and the dry run are
    /// the caller's: the run stages `--migration NAME` as it was given,
    /// and the check plans nothing (`dry_run`), its pair named
    /// [`PENDING_MIGRATION_NAME`].
    fn stage_options(&self, name: &str, dry_run: bool) -> StageOptions {
        StageOptions {
            schema: self.schema_dir.clone(),
            history: self.migrations_dir.clone(),
            name: name.to_owned(),
            renames: self.renames.clone(),
            allow_destructive: self.allow_destructive,
            dry_run,
        }
    }
}

/// The un-staged truth change a check reports as a finding of its own: a
/// staged history base is the last known schema, so a truth that differs
/// from the snapshot is a change the history has not taken yet. The run
/// plans the staging only (dry run; nothing is written) and reports the
/// would-be pair — its human line names the last staged migration and the
/// version the pair takes, and the `--json` field `pending_migration`
/// carries the same — rather than among the report's `missing` files
/// (deliberate; see `docs/unified-pipeline-plan.md`, ticket (c)).
///
/// There is nothing to look for until the history dir holds migrations,
/// and a truth equal to the snapshot stages nothing: the staging's
/// `Error::is_nothing_to_migrate` stop is that clean state, not a
/// failure. Any other staging problem fails the run with its message.
fn staged_drift(settings: &Settings) -> Result<Option<Pending>, String> {
    let history = rok_data_codegen::existing_migrations(&settings.migrations_dir)
        .map_err(|e| e.to_string())?;
    let Some(base) = history.last().map(|migration| migration.stem.clone()) else {
        return Ok(None);
    };
    let staged =
        match rok_data_codegen::stage(&settings.stage_options(PENDING_MIGRATION_NAME, true)) {
            Ok(staged) => staged,
            Err(error) if error.is_nothing_to_migrate() => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
    Ok(Some(Pending {
        base,
        version: staged.version,
        name: PENDING_MIGRATION_NAME.to_owned(),
    }))
}

/// The name a check's planned pair carries until it is staged for real.
const PENDING_MIGRATION_NAME: &str = "pending";

/// The runs a `rok gen` does: one per App a project file named, or the
/// one run of a plain project.
fn resolve(options: &GenOptions, check: bool) -> Result<Vec<Run>, String> {
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

/// Prints the numbered next steps.
fn next_steps(settings: &Settings, staged: Option<&Staged>) {
    let mut steps: Vec<String> = Vec::new();
    if let Some(staged) = staged {
        // The runtime's loader reads the history dir's pairs (and skips
        // the snapshot beside them); the --dir is the loader's own flag,
        // needed only when the run moved the history off its default.
        let dir = if settings.migrations_dir == Path::new(DEFAULT_MIGRATIONS_DIR) {
            String::new()
        } else {
            format!(" --dir {}", display_path(&settings.migrations_dir))
        };
        steps.push(format!(
            "apply the staged pair ({}) with `rok migrate up{dir}`",
            display_path(&staged.up)
        ));
    }
    steps.push("`rok gen check` fails the build when the generated code is out of date".to_owned());
    println!("\nNext steps:");
    for (number, step) in steps.iter().enumerate() {
        println!("  {}. {step}", number + 1);
    }
}
