//! `generate --watch`: generate, then regenerate whenever an input changes.
//!
//! The inputs (the schema paths, the query paths and the config file) are
//! polled: every [`POLL`], their files' sizes and modification times are
//! compared with the last look. A change waits until two looks agree, so an
//! editor's save, or a branch switch, regenerates once. Polling needs no
//! platform watcher, and the inputs are a handful of small files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;

use super::generate::{GenerateArgs, generate};
use super::print::print_report;
use crate::Config;
use crate::report::display_path;

/// How often the inputs are looked at, and how long they must stay
/// unchanged before a run.
const POLL: Duration = Duration::from_millis(250);

/// Each input file's size and modification time.
type Snapshot = BTreeMap<PathBuf, (u64, Option<SystemTime>)>;

/// Generates with `args`, then again on every change, until interrupted.
/// A failed run is printed and the watch goes on.
pub(super) fn watch(args: &GenerateArgs) -> Result<()> {
    let mut inputs = inputs(args, generate_once(args).as_ref());
    let shown: Vec<String> = inputs.iter().map(|path| display_path(path)).collect();
    println!("watching {} for changes (Ctrl-C stops)", shown.join(", "));
    let mut last = snapshot(&inputs);
    loop {
        thread::sleep(POLL);
        let mut seen = snapshot(&inputs);
        if seen == last {
            continue;
        }
        loop {
            thread::sleep(POLL);
            let settled = snapshot(&inputs);
            if settled == seen {
                break;
            }
            seen = settled;
        }
        let config = regenerate(args);
        inputs = inputs_or(args, config.as_ref(), inputs);
        last = snapshot(&inputs);
    }
}

/// The first run: the whole report, as without `--watch`.
fn generate_once(args: &GenerateArgs) -> Option<Config> {
    match args.resolve() {
        Ok(config) => {
            if let Err(err) = generate(&config) {
                eprintln!("error: {err:#}");
            }
            Some(config)
        }
        Err(err) => {
            eprintln!("error: {err:#}");
            None
        }
    }
}

/// A later run: its failures and warnings, then one line.
fn regenerate(args: &GenerateArgs) -> Option<Config> {
    let started = Instant::now();
    let config = match args.resolve() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("error: {err:#}");
            println!("the configuration does not load; watching for changes");
            return None;
        }
    };
    match crate::generate(&config) {
        Ok(report) => {
            let quiet = crate::Report {
                written: Vec::new(),
                scaffolded: Vec::new(),
                unchanged: Vec::new(),
                stale: Vec::new(),
                ..report.clone()
            };
            print_report(&config, &quiet);
            let elapsed = started.elapsed().as_millis();
            let (written, removed) = (report.written.len(), report.stale.len());
            if written + removed + report.scaffolded.len() == 0 {
                println!(
                    "up to date ({} files) in {elapsed} ms",
                    report.unchanged.len()
                );
            } else {
                println!(
                    "regenerated in {elapsed} ms: {written} written, {removed} removed, {} unchanged",
                    report.unchanged.len()
                );
            }
        }
        Err(err) => {
            print_report(&config, err.report());
            eprintln!("error: {err:#}");
            println!("generation failed; watching for changes");
        }
    }
    Some(config)
}

/// The paths a watch looks at: the config file, and the configured schema
/// and query paths.
fn inputs(args: &GenerateArgs, config: Option<&Config>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = args.config_file().ok().flatten().into_iter().collect();
    if let Some(config) = config {
        paths.extend(config.schema.iter().cloned());
        paths.extend(config.queries.iter().cloned());
    }
    paths
}

/// [`inputs`], or the previous ones when the configuration didn't load.
fn inputs_or(args: &GenerateArgs, config: Option<&Config>, previous: Vec<PathBuf>) -> Vec<PathBuf> {
    match config {
        Some(config) => inputs(args, Some(config)),
        None => previous,
    }
}

fn snapshot(paths: &[PathBuf]) -> Snapshot {
    let mut snapshot = Snapshot::new();
    for path in paths {
        look(path, &mut snapshot);
    }
    snapshot
}

/// Records `path`, and everything under it when it is a directory. A path
/// that is missing is recorded as such, so its creation is a change.
fn look(path: &Path, snapshot: &mut Snapshot) {
    let Ok(metadata) = std::fs::metadata(path) else {
        snapshot.insert(path.to_path_buf(), (0, None));
        return;
    };
    if metadata.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            look(&entry.path(), snapshot);
        }
    } else {
        snapshot.insert(
            path.to_path_buf(),
            (metadata.len(), metadata.modified().ok()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapshot_sees_edits_creations_and_removals_under_a_directory() -> Result<()> {
        let dir = tempfile::TempDir::new()?;
        let queries = dir.path().join("queries");
        std::fs::create_dir(&queries)?;
        std::fs::write(queries.join("a.sql"), "-- name: A :one\nSELECT 1;\n")?;
        let missing = dir.path().join("rok-data-sqlc.toml");
        let paths = [queries.clone(), missing.clone()];

        let before = snapshot(&paths);
        assert_eq!(before.len(), 2, "{before:?}");
        assert_eq!(snapshot(&paths), before, "nothing changed");

        std::fs::write(queries.join("a.sql"), "-- name: A :one\nSELECT 12;\n")?;
        let edited = snapshot(&paths);
        assert_ne!(edited, before);

        std::fs::write(queries.join("b.sql"), "")?;
        std::fs::write(&missing, "")?;
        let created = snapshot(&paths);
        assert_eq!(created.len(), 3, "{created:?}");

        std::fs::remove_file(queries.join("b.sql"))?;
        assert_eq!(snapshot(&paths).len(), 2);
        Ok(())
    }
}
