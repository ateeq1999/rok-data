//! `rok migrate create|status|up|down`: run `NNNN_name.up.sql` /
//! `NNNN_name.down.sql` migration files with the checksummed journal engine
//! in `rok-data` (see `rok_data::migration` and `rok_data::migration_file`).

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use rok_data::migration::{self, Migration};
use rok_data::migration_file;

use crate::context;
use crate::output::{json_line, status_json};

/// `rok migrate create NAME --dir migrations`.
///
/// Writes `NNNN_name.up.sql` (empty, ready to fill) with a matching
/// `.down.sql`, the next version greater than anything in the directory.
pub(crate) fn create(name: &str, dir: &Path) -> Result<ExitCode, String> {
    let name = assert_migration_name(name)?;
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let version = next_version(dir)?;
    let up = dir.join(format!("{version:04}_{name}.up.sql"));
    let down = dir.join(format!("{version:04}_{name}.down.sql"));
    fs::write(
        &up,
        format!("-- {name}\n\n\n-- down\n\n-- DROP TABLE IF EXISTS ...;\n"),
    )
    .map_err(|e| format!("{}: {e}", up.display()))?;
    fs::write(&down, format!("-- undo {name}\n"))
        .map_err(|e| format!("{}: {e}", down.display()))?;
    println!("Created: {}", up.display());
    println!("Created: {}", down.display());
    Ok(ExitCode::SUCCESS)
}

fn assert_migration_name(name: &str) -> Result<&str, String> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if valid {
        Ok(name)
    } else {
        Err(format!(
            "migration name `{name}`: use lowercase letters, digits and `_` \
             (like `create_forum_table`)"
        ))
    }
}

/// The next version number: `1` in an empty directory, one above the largest
/// numeric prefix seen (`rok-data`'s migration files order by version).
fn next_version(dir: &Path) -> Result<i64, String> {
    // Timestamp-prefixed files (the long form of `create`) also parse, so
    // their order is preserved and successors keep going above them.
    if let Some(hint) = migration_file::next_version_hint(dir) {
        return Ok(hint + 1);
    }
    // Fall back to today's timestamp only when the directory holds nothing
    // readable, so `create` in a fresh directory produces `0001_…`.
    Ok(1)
}

/// `rok migrate status`.
pub(crate) async fn status(
    dir: &Path,
    database_url: Option<&str>,
    json: bool,
) -> Result<ExitCode, String> {
    let connection = context::Connection::resolve(database_url)?;
    let db = connection.probe_into_db().await?;
    let migrations = load(dir)?;
    let applied = migration_file::applied_or_empty(&db)
        .await
        .map_err(|e| e.to_string())?;
    let rows = migration_file::status_rows(&migrations, &applied);
    if json {
        json_line(status_json(&rows));
    } else {
        engine_table(&rows);
    }
    Ok(ExitCode::SUCCESS)
}

/// `rok migrate up`.
pub(crate) async fn up(
    dir: &Path,
    database_url: Option<&str>,
    json: bool,
) -> Result<ExitCode, String> {
    let connection = context::Connection::resolve(database_url)?;
    let migrations = load(dir)?;
    let db = connection.probe_into_db().await?;
    let done = migration::apply(&db, &migrations)
        .await
        .map_err(|e| e.to_string())?;
    if json {
        let names: Vec<serde_json::Value> = done
            .iter()
            .map(|v| applied_names(&migrations, *v))
            .collect();
        json_line(serde_json::json!({ "applied": names }));
    } else {
        report(
            "applied",
            &done,
            &migrations,
            "nothing to apply: up to date",
        );
    }
    Ok(ExitCode::SUCCESS)
}

/// `rok migrate down [--steps N]`.
pub(crate) async fn down(
    dir: &Path,
    database_url: Option<&str>,
    steps: usize,
    json: bool,
) -> Result<ExitCode, String> {
    if steps == 0 {
        return Err("--steps N: expected a positive number".to_owned());
    }
    let connection = context::Connection::resolve(database_url)?;
    let migrations = load(dir)?;
    let db = connection.probe_into_db().await?;
    let done = migration::revert(&db, &migrations, steps)
        .await
        .map_err(|e| e.to_string())?;
    if json {
        let names: Vec<serde_json::Value> = done
            .iter()
            .map(|v| applied_names(&migrations, *v))
            .collect();
        json_line(serde_json::json!({ "reverted": names }));
    } else {
        report("reverted", &done, &migrations, "nothing to revert");
    }
    Ok(ExitCode::SUCCESS)
}

fn applied_names(migrations: &[Migration], version: i64) -> serde_json::Value {
    let name = migrations
        .iter()
        .find(|m| m.version == version)
        .map_or("", |m| m.name);
    serde_json::json!({ "version": version, "name": name })
}

fn report(verb: &str, versions: &[i64], migrations: &[Migration], none: &str) {
    if versions.is_empty() {
        println!("{none}");
    }
    for version in versions {
        let name = migrations
            .iter()
            .find(|m| m.version == *version)
            .map_or("", |m| m.name);
        println!("{verb} {version} {name}");
    }
}

fn engine_table(rows: &[migration_file::StatusRow]) {
    let width = rows.iter().map(|r| r.name.len()).max().unwrap_or(0).max(4);
    println!(
        "{:<8} {:<width$} {:<8} checksum",
        "version", "name", "state"
    );
    for row in rows {
        println!(
            "{:<8} {:<width$} {:<8} {}",
            row.version, row.name, row.state, row.checksum
        );
    }
}

/// The directory must contain exactly the `<version>_<name>` pair convention.
/// See `rok_data::migration_file::load` for the guarantees.
fn load(dir: &Path) -> Result<Vec<Migration>, String> {
    migration_file::load(dir).map_err(|e| e.to_string())
}
