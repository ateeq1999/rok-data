//! The report of a `rok gen` run: what was written, what was skipped, what
//! was refused — as prose, or as one JSON object.

use rok_data_codegen::Staged;
use rok_data_sqlc::Report;
use serde_json::{Value, json};

use super::Settings;
use crate::output::display_path;

/// Prints what the run did: the skipped tables and queries and the
/// warnings on stderr, the files on stdout.
pub(super) fn print_report(settings: &Settings, report: &Report, staged: Option<&Staged>) {
    for failure in &report.table_failures {
        eprintln!(
            "warning: table `{}` skipped: {}",
            failure.table, failure.reason
        );
    }
    for failure in &report.query_failures {
        eprintln!(
            "warning: query `{}` skipped: {}",
            failure.query, failure.reason
        );
    }
    for warning in &report.warnings {
        eprintln!("warning: {warning}");
    }
    for path in &report.written {
        if settings.engine.dry_run {
            println!("would write {}", display_path(path));
        } else {
            println!("generated {}", display_path(path));
        }
    }
    for path in &report.scaffolded {
        if settings.engine.dry_run {
            println!("would scaffold {}", display_path(path));
        } else {
            println!("scaffolded {}", display_path(path));
        }
    }
    for path in &report.unchanged {
        // A check run lists every planned file; only the drift is news.
        if settings.engine.check {
            continue;
        }
        println!("unchanged {}", display_path(path));
    }
    for path in &report.missing {
        println!("missing {}", display_path(path));
    }
    for path in &report.changed {
        println!("changed {}", display_path(path));
    }
    for path in &report.stale {
        if settings.engine.check {
            println!("stale {}", display_path(path));
        } else if settings.engine.dry_run {
            println!("would remove {}", display_path(path));
        } else {
            println!("removed {}", display_path(path));
        }
    }
    for refused in &report.refused {
        let (path, reason) = (display_path(&refused.path), &refused.reason);
        if settings.engine.dry_run {
            println!("would refuse {path} ({reason})");
        } else {
            eprintln!("error: refused to write {path} ({reason})");
        }
    }
    if let Some(staged) = staged {
        let verb = if settings.engine.dry_run {
            "would stage"
        } else {
            "staged"
        };
        println!("migration `{}`:", staged.name);
        for step in &staged.steps {
            println!("  {step}");
        }
        println!("{verb} {}", display_path(&staged.up));
        if let Some(down) = &staged.down {
            println!("{verb} {}", display_path(down));
        }
    }
}

/// Prints the run's report as one JSON object.
pub(super) fn print_json(
    settings: &Settings,
    report: &Report,
    staged: Option<&Staged>,
    drift: bool,
) {
    let paths = |paths: &[std::path::PathBuf]| -> Vec<String> {
        paths.iter().map(|path| display_path(path)).collect()
    };
    let mut skipped: Vec<Value> = Vec::new();
    for failure in &report.table_failures {
        skipped.push(json!({
            "kind": "table",
            "name": failure.table,
            "reason": failure.reason,
        }));
    }
    for failure in &report.query_failures {
        skipped.push(json!({
            "kind": "query",
            "name": failure.query,
            "reason": failure.reason,
        }));
    }
    let refused: Vec<Value> = report
        .refused
        .iter()
        .map(|refused| {
            json!({
                "path": display_path(&refused.path),
                "reason": refused.reason,
            })
        })
        .collect();
    let migration = staged.map(|staged| {
        json!({
            "version": staged.version,
            "name": staged.name,
            "up": display_path(&staged.up),
            "down": staged.down.as_ref().map(|down| display_path(down)),
            "reversible": staged.reversible,
            "steps": staged.steps,
        })
    });
    let body = json!({
        "written": paths(&report.written),
        "scaffolded": paths(&report.scaffolded),
        "unchanged": paths(&report.unchanged),
        "refused": refused,
        "stale": paths(&report.stale),
        "missing": paths(&report.missing),
        "changed": paths(&report.changed),
        "skipped": skipped,
        "warnings": report.warnings,
        "migration": migration,
        "drift": drift,
        "dry_run": settings.engine.dry_run,
    });
    println!("{body}");
}
