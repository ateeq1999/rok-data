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

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rok_data_codegen::{Rename, StageOptions, Staged};
use rok_data_sqlc::{Config, Project};
use serde_json::Value;

use crate::cli::r#gen::{GenAction, GenOptions};
use crate::output::display_path;

mod clients;
mod report;
mod resolve;
mod steps;

use report::{print_json_body, print_report};
use resolve::resolve;
use steps::Steps;

/// The truth dir a project has when nothing else is said: also where the
/// pre-split layout (`rok init` before the history was separated) keeps
/// its staged pairs.
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
    /// The line naming the settings file and directories the run reads.
    using: String,
    /// The directory of the run's App (or plain project).
    dir: PathBuf,
}

/// `rok gen`: the data layer and the API features, and the migration pair
/// each run stages when `--migration NAME` is given.
fn generate(options: GenOptions) -> Result<ExitCode, String> {
    let runs = resolve(&options, false)?;
    let clients = clients::load(&options)?;
    let multi = runs.iter().any(|run| run.app.is_some()) || clients.is_some();
    let mut bodies: Vec<Value> = Vec::new();
    // A step that fails ends the run with what was done, what failed and
    // what was skipped; nothing done is rolled back.
    let mut steps = Steps::plan(&runs, clients.as_ref(), &options);
    for run in &runs {
        heading(run, multi);
        match rok_data_sqlc::generate(&run.settings.engine) {
            Ok(report) => {
                steps.done();
                let staged = stage(&run.settings);
                let shown = staged.as_ref().ok().and_then(Option::as_ref);
                if run.settings.json {
                    bodies.push(print_json_body(
                        &run.settings,
                        run.app.as_deref(),
                        &report,
                        shown,
                        None,
                        false,
                    ));
                } else {
                    print_report(&run.settings, &report, shown, None);
                }
                match staged {
                    Ok(staged) => {
                        if staged.is_some() {
                            steps.done();
                        }
                        if !run.settings.json {
                            next_steps(run, staged.as_ref());
                        }
                    }
                    Err(message) => return steps.fail(message, bodies, multi),
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
                return steps.fail(super::chain(&error), bodies, multi);
            }
        }
    }
    // A monorepo's Clients follow its Apps: the contract is re-exported
    // from the regenerated api, then each client is emitted from it.
    if let Some(project) = &clients
        && let Err(message) = clients::generate(project, &options, &mut bodies, &mut steps)
    {
        return steps.fail(message, bodies, multi);
    }
    print_bodies_then(bodies, multi, Ok(ExitCode::SUCCESS))
}

/// `rok gen check`: the CI mode. Nothing is written; each run's drift
/// (missing, changed, or stale files) and its un-staged truth change
/// ([`staged_drift`]) are reported, and either fails the run.
fn check(options: GenOptions) -> Result<ExitCode, String> {
    let runs = resolve(&options, true)?;
    let clients = clients::load(&options)?;
    let multi = runs.iter().any(|run| run.app.is_some()) || clients.is_some();
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
                    return print_bodies_then(bodies, multi, Err(super::chain(&error)));
                }
                findings.push(out_of_date_message(run));
                continue;
            }
        };
        let pending = staged_drift(&run.settings)?;
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
        if pending.is_some() {
            findings.push(pending_message(run));
        }
    }
    if let Some(project) = &clients {
        match clients::check(project, &options, &mut bodies) {
            Ok(found) => findings.extend(found),
            Err(message) => return print_bodies_then(bodies, multi, Err(message)),
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
    // On stderr, so `--json` stdout stays one document.
    eprintln!("{}", run.using);
    if multi
        && !run.settings.json
        && let Some(app) = &run.app
    {
        println!("{app}:");
    }
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
        .map_err(crate::commands::chain)?;
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
        .map_err(crate::commands::chain)?;
    let Some(base) = history.last().map(|migration| migration.stem.clone()) else {
        return Ok(None);
    };
    let staged =
        match rok_data_codegen::stage(&settings.stage_options(PENDING_MIGRATION_NAME, true)) {
            Ok(staged) => staged,
            Err(error) if error.is_nothing_to_migrate() => return Ok(None),
            Err(error) => return Err(crate::commands::chain(error)),
        };
    Ok(Some(Pending {
        base,
        version: staged.version,
        name: PENDING_MIGRATION_NAME.to_owned(),
    }))
}

/// What `rok migrate up` needs to read the run's history dir from where
/// `rok gen` ran: nothing when the project rule finds it there (the App
/// started in, a Monorepo's one App, a plain project's settings), `--app`
/// at a root naming several Apps, and `--dir` for a history dir only a
/// flag named.
fn migrate_place(run: &Run) -> String {
    let found = |app: Option<&str>| {
        Project::current()
            .and_then(|project| project.target(app))
            .and_then(|target| target.migrations_dir())
            .is_ok_and(|dir| same_dir(&dir, &run.settings.migrations_dir))
    };
    if found(None) {
        String::new()
    } else if let Some(app) = run.app.as_deref().filter(|app| found(Some(app))) {
        format!(" --app {app}")
    } else {
        format!(" --dir {}", display_path(&run.settings.migrations_dir))
    }
}

/// Whether two paths name one directory.
fn same_dir(a: &Path, b: &Path) -> bool {
    let full = |path: &Path| {
        path.canonicalize()
            .or_else(|_| std::path::absolute(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    full(a) == full(b)
}

/// The name a check's planned pair carries until it is staged for real.
const PENDING_MIGRATION_NAME: &str = "pending";

/// Prints the numbered next steps.
///
/// Each command runs as printed from the directory `rok gen` ran in, and
/// each path is relative to it.
fn next_steps(run: &Run, staged: Option<&Staged>) {
    let mut steps: Vec<String> = Vec::new();
    if let Some(staged) = staged {
        steps.push(format!(
            "apply the staged pair ({}) with `rok migrate up{}`",
            display_path(&staged.up),
            migrate_place(run)
        ));
    }
    steps.push("`rok gen check` fails the build when the generated code is out of date".to_owned());
    println!("\nNext steps:");
    for (number, step) in steps.iter().enumerate() {
        println!("  {}. {step}", number + 1);
    }
}
