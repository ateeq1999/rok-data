//! `rok schema dump` and `rok schema diff`.
//!
//! Ported from the old `rok` CLI of `rok-fluent` (schema dump lives in a
//! library now: `rok_data::introspect`), with the journal check updated to
//! the unified `_rok_data_migrations` engine.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use rok_data::Db;
use rok_data::migration_file;

use crate::context;

/// `rok schema dump`: approximate DDL for every user table.
pub(crate) async fn dump(database_url: Option<&str>) -> Result<ExitCode, String> {
    let connection = context::Connection::resolve(database_url)?;
    let db = connection.probe_into_db().await?;
    dump_for(&db).await
}

async fn dump_for(db: &Db) -> Result<ExitCode, String> {
    let tables = rok_data::introspect::tables(db)
        .await
        .map_err(|e| e.to_string())?;
    if tables.is_empty() {
        println!("-- No user tables found in schema 'public'.");
        return Ok(ExitCode::SUCCESS);
    }
    println!("-- rok schema dump  (approximate DDL - not for round-trip use)");
    for table in tables {
        let ddl = rok_data::introspect::dump_table(db, &table)
            .await
            .map_err(|e| e.to_string())?;
        println!();
        println!("{ddl}");
    }
    Ok(ExitCode::SUCCESS)
}

/// `rok schema diff [--dir migrations]`: journal rows on one side, migration
/// files on the other. A coarser view than `rok migrate status` (no
/// checksums) kept from the old CLI.
pub(crate) async fn diff(dir: &Path, database_url: Option<&str>) -> Result<ExitCode, String> {
    let connection = context::Connection::resolve(database_url)?;
    let db = connection.probe_into_db().await?;
    // Applied migrations — gracefully handle a never-migrated database.
    let applied = migration_file::applied_or_empty(&db)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|m| m.name)
        .collect::<Vec<_>>();
    let files = file_names(dir);

    println!("{:<10} Migration", "Status");
    println!("{}", "-".repeat(60));
    for (version, name) in &files {
        let applied_here = applied.iter().any(|n| n == name);
        let status = if applied_here { "Applied" } else { "Pending" };
        println!("{status:<10} {version:04}_{name}");
    }
    // Orphans: in the journal but not on disk.
    let on_disk: Vec<&str> = files.iter().map(|(_, name)| name.as_str()).collect();
    for name in &applied {
        if !on_disk.contains(&name.as_str()) {
            println!("{:<10} {name}", "Orphan");
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Migration files in `dir` as `(version, name)`, oldest first.
///
/// Lenient on purpose: unlike `rok migrate` (which validates via
/// `rok_data::migration_file::load`), `schema diff` only *reports* what it
/// sees, the way the old `rok` CLI did before the consolidation.
pub(crate) fn file_names(dir: &Path) -> Vec<(i64, String)> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            let stem = file
                .strip_suffix(".up.sql")
                .or_else(|| file.strip_suffix(".sql"))
                .unwrap_or(&file);
            if let Some((version, name)) = stem.split_once('_')
                && let Ok(version) = version.parse::<i64>()
                && !name.is_empty()
            {
                out.push((version, name.to_owned()));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
