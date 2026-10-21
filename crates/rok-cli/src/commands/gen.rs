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

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rok_data_codegen::{Rename, StageOptions, Staged};
use rok_data_sqlc::{Config, Report};

use crate::cli::{GenAction, GenOptions};
use crate::output::display_path;

mod report;

use report::{print_json, print_report};

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

/// `rok gen`: the data layer and the API features, and the migration pair
/// when `--migration NAME` is given.
fn generate(options: GenOptions) -> Result<ExitCode, String> {
    let settings = Settings::resolve(&options, false)?;
    let report = match rok_data_sqlc::generate(&settings.engine) {
        Ok(report) => report,
        Err(error) => {
            // The report says what was known when the run stopped.
            if settings.json {
                print_json(&settings, error.report(), None, None, false);
            } else {
                print_report(&settings, error.report(), None, None);
            }
            return Err(error.to_string());
        }
    };
    let strict = strict_failure(&settings, &report);
    let staged = if strict.is_none() {
        stage(&settings)?
    } else {
        None
    };
    if settings.json {
        print_json(&settings, &report, staged.as_ref(), None, false);
    } else {
        print_report(&settings, &report, staged.as_ref(), None);
        next_steps(&settings, staged.as_ref());
    }
    match strict {
        Some(message) => Err(message),
        None => Ok(ExitCode::SUCCESS),
    }
}

/// `rok gen check`: the CI mode. Nothing is written; the drift (missing,
/// changed, or stale files) and an un-staged truth change
/// ([`staged_drift`]) are reported, and either fails the run.
fn check(options: GenOptions) -> Result<ExitCode, String> {
    let settings = Settings::resolve(&options, true)?;
    let report = match rok_data_sqlc::generate(&settings.engine) {
        Ok(report) => report,
        Err(error) => {
            let report = error.report();
            let drift = report.has_drift();
            if settings.json {
                print_json(&settings, report, None, None, drift);
            } else {
                print_report(&settings, report, None, None);
            }
            if !drift {
                // Not a drift: the run failed for another reason.
                return Err(error.to_string());
            }
            eprintln!("error: the generated code is out of date; run `rok gen` to update it");
            return Ok(ExitCode::FAILURE);
        }
    };
    let strict = strict_failure(&settings, &report);
    let pending = if strict.is_none() {
        staged_drift(&settings)?
    } else {
        None
    };
    if settings.json {
        print_json(&settings, &report, None, pending.as_ref(), false);
    } else {
        print_report(&settings, &report, None, pending.as_ref());
        if !report.has_drift() && pending.is_none() {
            println!("the generated code is up to date");
        }
    }
    match strict {
        Some(message) => Err(message),
        None if pending.is_some() => {
            eprintln!(
                "error: the truth has a change that is not staged as a migration; \
                 run `rok gen --migration NAME` in the repo"
            );
            Ok(ExitCode::FAILURE)
        }
        None => Ok(ExitCode::SUCCESS),
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
    /// Resolves the settings: the config file (the flag, or the discovered
    /// one) and the flags over it.
    fn resolve(options: &GenOptions, check: bool) -> Result<Settings, String> {
        let file = match options.config.as_deref() {
            Some(path) => Some(path.to_path_buf()),
            None => std::env::current_dir()
                .ok()
                .and_then(|dir| rok_data_sqlc::discover_config(&dir)),
        };
        let mut engine = match &file {
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
            None if engine.schema.is_empty() => vec![PathBuf::from(DEFAULT_SCHEMA_DIR)],
            None => engine.schema.clone(),
        };
        let schema_dir = match options.schema.as_deref() {
            Some(dir) => dir.to_path_buf(),
            None => entries
                .iter()
                .find(|entry| entry.is_dir())
                .cloned()
                .or_else(|| entries.first().cloned())
                .unwrap_or_else(|| PathBuf::from(DEFAULT_SCHEMA_DIR)),
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
                Rename::parse(spec).ok_or_else(|| {
                    format!("--rename `{spec}`: expected `old=new` or `table.old=new`")
                })
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
