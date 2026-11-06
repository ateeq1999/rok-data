//! The Clients step of a monorepo's `rok gen` (workspace ADR 0027): after
//! the Apps regenerate, the contract is re-exported (`[contract]`'s
//! `export`, which builds the api App) and every client `[clients]` names
//! is emitted from it by the engine, with its generated-file rules —
//! changed files rewritten, a file whose marker is gone refused without
//! `--force`, files no longer emitted removed as stale. `rok gen check`
//! builds and writes nothing: each client is emitted in memory from the
//! document as it is, and any missing, changed or stale file is a finding.
//! A dry run neither exports nor writes, and reports what would change.

use std::path::Path;

use rok_data_sqlc::clients::{self, Mode, Outcome, Project};
use serde_json::{Value, json};

use super::Steps;
use super::resolve::project;
use crate::cli::r#gen::GenOptions;
use crate::config::FILE;
use crate::output::display_path;

/// The contract and Clients the run's project file names, when it is the
/// canonical `rok.toml` and names any.
pub(super) fn load(options: &GenOptions) -> Result<Option<Project>, String> {
    let found = project(options)?;
    let Some(file) = found.file() else {
        return Ok(None);
    };
    if file.file_name().and_then(|name| name.to_str()) != Some(FILE) {
        return Ok(None);
    }
    Project::load(file).map_err(|error| format!("{error:#}"))
}

/// `rok gen`'s step: re-export the contract (not in a dry run), then emit
/// every client, each a step of `steps`. Prose goes to stdout; with
/// `--json`, one body per client joins `bodies`. The error is the failed
/// step's.
pub(super) fn generate(
    project: &Project,
    options: &GenOptions,
    bodies: &mut Vec<Value>,
    steps: &mut Steps<'_>,
) -> Result<(), String> {
    let export = !options.dry_run && project.export.is_some();
    if !options.json {
        println!("clients:");
    }
    if export {
        // The export builds the App: say so first (on stderr, so `--json`'s
        // stdout stays one document), or a slow first build is silent.
        eprintln!("exporting the contract (building the App, slow the first time)\u{2026}");
        project.export().map_err(|error| format!("{error:#}"))?;
        steps.done();
        if !options.json {
            println!("exported {}", display_path(&project.document));
        }
    }
    let mode = Mode::Write {
        dry_run: options.dry_run,
        force: options.force,
    };
    // A contract that can't be read fails the first client's step.
    let document = project
        .read_document()
        .map_err(|error| format!("{error:#}"))?;
    for client in &project.clients {
        let outcome =
            clients::emit_client(&document, client, mode).map_err(|error| format!("{error:#}"))?;
        steps.done();
        if options.json {
            bodies.push(body(project, &outcome, export.then_some(&project.document)));
        } else {
            print(&outcome, options.dry_run, false);
        }
    }
    Ok(())
}

/// `rok gen check`'s step: each client against the document as it is.
/// Returns a finding per client out of date.
pub(super) fn check(
    project: &Project,
    options: &GenOptions,
    bodies: &mut Vec<Value>,
) -> Result<Vec<String>, String> {
    if !options.json {
        println!("clients:");
    }
    let outcomes = clients::generate(project, Mode::Check).map_err(|error| format!("{error:#}"))?;
    let mut findings = Vec::new();
    for outcome in &outcomes {
        if options.json {
            bodies.push(body(project, outcome, None));
        } else {
            print(outcome, false, true);
        }
        if outcome.has_drift() {
            findings.push(format!(
                "the `{}` client is out of date with {}; run `rok gen` to update it",
                outcome.client,
                display_path(&project.document)
            ));
        }
    }
    Ok(findings)
}

/// One client's files, as prose.
fn print(outcome: &Outcome, dry_run: bool, check: bool) {
    for path in &outcome.written {
        let verb = if dry_run { "would write" } else { "generated" };
        println!("{verb} {}", display_path(path));
    }
    if !check {
        for path in &outcome.unchanged {
            println!("unchanged {}", display_path(path));
        }
    }
    for path in &outcome.missing {
        println!("missing {}", display_path(path));
    }
    for path in &outcome.changed {
        println!("changed {}", display_path(path));
    }
    for path in &outcome.stale {
        let verb = if check {
            "stale"
        } else if dry_run {
            "would remove"
        } else {
            "removed"
        };
        println!("{verb} {}", display_path(path));
    }
}

/// One client's files, as a `--json` body beside the Apps'.
fn body(project: &Project, outcome: &Outcome, exported: Option<&Path>) -> Value {
    let paths = |paths: &[std::path::PathBuf]| -> Vec<String> {
        paths.iter().map(|path| display_path(path)).collect()
    };
    json!({
        "client": outcome.client.key(),
        "dir": display_path(&outcome.dir),
        "document": display_path(&project.document),
        "exported": exported.map(display_path),
        "written": paths(&outcome.written),
        "unchanged": paths(&outcome.unchanged),
        "missing": paths(&outcome.missing),
        "changed": paths(&outcome.changed),
        "stale": paths(&outcome.stale),
        "drift": outcome.has_drift(),
    })
}
