//! `rok gen` / `rok gen check`: the unified front door.
//!
//! Two things, both over the project's schema dir:
//!
//! 1. the engine's generator (`rok_data_sqlc`): the data layer in
//!    `src/db/`, the shared tools, and the API features in `src/features/`;
//! 2. the codegen crate's migration staging, used only with `--migration
//!    NAME`: the difference between the schema dir and the state of the last
//!    staged migration, written as a `NNNN_name.up.sql` pair.
//!
//! The schema dir is the project's only source of truth (ADR 0010). It
//! holds the truth (plain `*.sql` files) and the history (the staged
//! migration pairs); the generator reads the truth only, so a staged
//! migration is never applied twice.

use std::path::PathBuf;
use std::process::ExitCode;

use rok_data_codegen::{Rename, StageOptions, Staged};
use rok_data_sqlc::{Config, Report};

use crate::cli::{GenAction, GenCheckOptions, GenOptions};
use crate::output::display_path;

mod report;

use report::{print_json, print_report};

/// The schema dir a project has when nothing else is said: the one `rok
/// init` seeds and the runtime's migrations live in.
const DEFAULT_SCHEMA_DIR: &str = "migrations";

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
    let GenCheckOptions {
        config,
        schema,
        output,
        resource,
        queries,
        allow_destructive,
        strict,
        json,
        dry_run,
        force,
    } = leaf;
    check(GenOptions {
        config: config.or(options.config),
        schema: schema.or(options.schema),
        output: output.or(options.output),
        resource: resource.or(options.resource),
        queries: if queries.is_empty() {
            options.queries
        } else {
            queries
        },
        migration: None,
        allow_destructive: allow_destructive || options.allow_destructive,
        renames: Vec::new(),
        strict: strict || options.strict,
        json: json || options.json,
        dry_run: dry_run || options.dry_run,
        force: force || options.force,
    })
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
                print_json(&settings, error.report(), None, false);
            } else {
                print_report(&settings, error.report(), None);
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
        print_json(&settings, &report, staged.as_ref(), false);
    } else {
        print_report(&settings, &report, staged.as_ref());
        next_steps(&settings, staged.as_ref());
    }
    match strict {
        Some(message) => Err(message),
        None => Ok(ExitCode::SUCCESS),
    }
}

/// `rok gen check`: the CI mode. Nothing is written; the drift (missing,
/// changed, or stale files) is reported and fails the run.
fn check(options: GenOptions) -> Result<ExitCode, String> {
    let settings = Settings::resolve(&options, true)?;
    let report = match rok_data_sqlc::generate(&settings.engine) {
        Ok(report) => report,
        Err(error) => {
            let report = error.report();
            let drift = report.has_drift();
            if settings.json {
                print_json(&settings, report, None, drift);
            } else {
                print_report(&settings, report, None);
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
    if settings.json {
        print_json(&settings, &report, None, false);
    } else {
        print_report(&settings, &report, None);
        println!("the generated code is up to date");
    }
    match strict {
        Some(message) => Err(message),
        None => Ok(ExitCode::SUCCESS),
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
    let options = StageOptions {
        schema: settings.schema_dir.clone(),
        name: name.clone(),
        renames: settings.renames.clone(),
        allow_destructive: settings.allow_destructive,
        dry_run: settings.engine.dry_run,
    };
    let staged = rok_data_codegen::stage(&options).map_err(|e| e.to_string())?;
    Ok(Some(staged))
}

/// The run's settings: the config file's values with the flags over them.
struct Settings {
    /// The engine's configuration.
    engine: Config,
    /// The schema dir the migration staging works in.
    schema_dir: PathBuf,
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
        // The flags beat the file (ADR 0011).
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
        Ok(Settings {
            engine,
            schema_dir,
            migration: options.migration.clone(),
            renames,
            allow_destructive: options.allow_destructive,
            strict: options.strict,
            json: options.json,
        })
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
    if staged.is_some() {
        steps.push(format!(
            "apply the migrations: `rok migrate up --dir {}`",
            display_path(&settings.schema_dir)
        ));
    }
    steps.push("`rok gen check` fails the build when the generated code is out of date".to_owned());
    println!("\nNext steps:");
    for (number, step) in steps.iter().enumerate() {
        println!("  {}. {step}", number + 1);
    }
}
