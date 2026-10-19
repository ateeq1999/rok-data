//! Migrations as plain files: `NNNN_name.up.sql` with an optional
//! `NNNN_name.down.sql`, read from a directory and fed to the same
//! checksummed engine as code-generated migrations (see [`crate::migration`]).
//!
//! Files live in one directory, are ordered by their numeric version prefix
//! and are recorded in the `_rok_data_migrations` journal like any other
//! migration, so `up`, `down` and drift detection behave identically whether
//! the source of the files is an editor or the generator.
//!
//! ```text
//! migrations/
//! ├── 0001_init.up.sql
//! ├── 0001_init.down.sql
//! └── 0002_add_slug.up.sql
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::migration::{AppliedMigration, Migration};
use crate::{Db, raw};

/// Whether the migration journal table (`_rok_data_migrations`) exists in the
/// database yet.
///
/// [`StatusRow`] reporting uses this so that `status` can look at a database
/// that has never been migrated without creating the table as a side effect:
/// a database with no journal simply has every migration pending.
pub async fn journal_exists(db: &Db) -> crate::Result<bool> {
    raw("SELECT to_regclass('_rok_data_migrations') IS NOT NULL")
        .scalar(db)
        .await
}

/// Every recorded migration, or an empty list when the journal table does not
/// exist yet (a database that was never migrated has no applied migrations).
pub async fn applied_or_empty(db: &Db) -> crate::Result<Vec<AppliedMigration>> {
    if journal_exists(db).await? {
        crate::migration::applied(db).await
    } else {
        Ok(Vec::new())
    }
}

/// Status of one migration in `status()` output.
#[derive(Debug, PartialEq, Eq)]
pub struct StatusRow {
    /// Numeric version from the file name (`0001` for `0001_init.up.sql`).
    pub version: i64,
    /// Migration name from the file name (`init` for `0001_init.up.sql`).
    pub name: String,
    /// `"applied"` or `"pending"`.
    pub state: &'static str,
    /// `"ok"` when the recorded checksum matches the file, empty for pending.
    pub checksum: &'static str,
    /// `true` when the journal and the files disagree: this migration was
    /// edited after being applied, or exists only in the journal.
    pub drift: bool,
}

/// Compare file-applied migrations with the journal: every file becomes one
/// row (`applied`/`pending`) and journal rows without a file become `unknown`.
pub fn status_rows(migrations: &[Migration], applied: &[AppliedMigration]) -> Vec<StatusRow> {
    let mut rows = BTreeMap::new();
    for m in migrations {
        let row = match applied.iter().find(|a| a.version == m.version) {
            Some(a) if a.checksum == m.checksum() => ("applied", "ok", false),
            Some(_) => ("applied", "edited", true),
            None => ("pending", "", false),
        };
        rows.insert(m.version, (m.name.to_owned(), row));
    }
    for a in applied {
        rows.entry(a.version)
            .or_insert_with(|| (a.name.clone(), ("unknown", "", true)));
    }
    rows.into_iter()
        .map(|(version, (name, (state, checksum, drift)))| StatusRow {
            version,
            name,
            state,
            checksum,
            drift,
        })
        .collect()
}

/// Read `NNNN_name.up.sql` (and the optional `NNNN_name.down.sql`) from `dir`,
/// oldest first. Errors when the directory mixes in files that do not follow
/// the `<version>_<name>` convention, has duplicate versions, or has a down
/// file without its matching up file.
///
/// The contents are exactly what the generated crate embeds with
/// `include_str!`, so checksums match a generated crate's constants.
pub fn load(dir: &Path) -> Result<Vec<Migration>, crate::Error> {
    let entries =
        fs::read_dir(dir).map_err(|e| crate::Error::Config(format!("{}: {e}", dir.display())))?;
    let mut ups = BTreeMap::new();
    let mut downs = BTreeMap::new();
    for entry in entries {
        let entry = entry.map_err(|e| crate::Error::Config(format!("{}: {e}", dir.display())))?;
        let file = entry.file_name().to_string_lossy().into_owned();
        let (stem, map) = if let Some(stem) = file.strip_suffix(".up.sql") {
            (stem, &mut ups)
        } else if let Some(stem) = file.strip_suffix(".down.sql") {
            (stem, &mut downs)
        } else {
            return Err(crate::Error::Config(format!(
                "{}: expected only `NNNN_name.up.sql` and `NNNN_name.down.sql` files",
                entry.path().display()
            )));
        };
        let version = stem
            .split_once('_')
            .and_then(|(v, name)| Some((v.parse::<i64>().ok()?, name)))
            .filter(|(_, name)| !name.is_empty());
        let Some((version, name)) = version else {
            return Err(crate::Error::Config(format!(
                "{}: the name must be `<version>_<name>`, like `0001_init.up.sql`",
                entry.path().display()
            )));
        };
        let sql = fs::read_to_string(entry.path())
            .map_err(|e| crate::Error::Config(format!("{}: {e}", entry.path().display())))?;
        if map.insert(version, (name.to_owned(), sql)).is_some() {
            return Err(crate::Error::Config(format!(
                "{}: two migrations have version {version}",
                dir.display()
            )));
        }
    }
    if let Some((version, (name, _))) = downs.iter().find(|(v, _)| !ups.contains_key(*v)) {
        return Err(crate::Error::Config(format!(
            "{}: `{version:04}_{name}.down.sql` has no matching `.up.sql`",
            dir.display()
        )));
    }
    if ups.is_empty() {
        return Err(crate::Error::Config(format!(
            "{} has no migrations; create one with `rok migrate create` or run `rok generate` first",
            dir.display()
        )));
    }
    // `Migration` holds `&'static str`, like the `include_str!` constants of
    // the generated crate; this is a one-shot process, so leaking keeps the
    // lifetime valid for the rest of it.
    let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
    Ok(ups
        .into_iter()
        .map(|(version, (name, up))| Migration {
            version,
            name: leak(name),
            up: leak(up),
            down: leak(
                downs
                    .remove(&version)
                    .map(|(_, down)| down)
                    .unwrap_or_default(),
            ),
        })
        .collect())
}

/// The next free version number for `dir`: the highest numeric prefix on
/// disk (`0003` → `4`), or `1` for an empty or unreadable directory.
///
/// Files without a `<version>_<name>.sql`-shaped suffix (notes and scratch
/// files) are ignored, so `rok migrate create` keeps counting correctly.
pub fn next_version_hint(dir: &Path) -> Option<i64> {
    let entries = fs::read_dir(dir).ok()?;
    let mut max = 0;
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        let stem = if file.ends_with(".up.sql") {
            file.trim_end_matches(".up.sql")
        } else if file.ends_with(".down.sql") {
            file.trim_end_matches(".down.sql")
        } else if file.ends_with(".sql") {
            file.trim_end_matches(".sql")
        } else {
            continue;
        };
        if let Some((v, _)) = stem.split_once('_')
            && let Ok(v) = v.parse::<i64>()
        {
            max = max.max(v);
        }
    }
    (max > 0).then_some(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rok-migration-file-{}-{}",
            std::process::id(),
            files.iter().map(|(f, _)| f.len()).sum::<usize>() + files.len()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (file, sql) in files {
            fs::write(dir.join(file), sql).unwrap();
        }
        dir
    }

    #[test]
    fn loads_pairs_in_version_order() {
        let dir = dir_with(&[
            ("0002_add_slug.up.sql", "ALTER 2"),
            ("0001_init.up.sql", "CREATE 1"),
            ("0001_init.down.sql", "DROP 1"),
        ]);
        let m = load(&dir).unwrap();
        assert_eq!(
            m.iter()
                .map(|m| (m.version, m.name, m.up, m.down))
                .collect::<Vec<_>>(),
            [
                (1, "init", "CREATE 1", "DROP 1"),
                (2, "add_slug", "ALTER 2", "")
            ]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_unexpected_files() {
        for files in [
            &[("0001_init.up.sql", ""), ("notes.txt", "")][..],
            &[("init.up.sql", "")][..],
            &[("0001_.up.sql", "")][..],
            &[("0001_init.up.sql", ""), ("0002_x.down.sql", "")][..],
            &[("0001_a.up.sql", ""), ("1_b.up.sql", "")][..],
            &[][..],
        ] {
            let dir = dir_with(files);
            assert!(load(&dir).is_err(), "{files:?}");
            fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn status_marks_drift() {
        let m = |version, up| Migration {
            version,
            name: "m",
            up,
            down: "",
        };
        let applied = |version, up: &'static str| AppliedMigration {
            version,
            name: "old".into(),
            checksum: m(version, up).checksum(),
        };
        let files = [m(1, "a"), m(2, "b"), m(3, "c")];
        let rows = status_rows(
            &files,
            &[applied(1, "a"), applied(2, "edited"), applied(9, "z")],
        );
        let states: Vec<_> = rows
            .iter()
            .map(|r| (r.version, r.state, r.checksum, r.drift))
            .collect();
        assert_eq!(
            states,
            [
                (1, "applied", "ok", false),
                (2, "applied", "edited", true),
                (3, "pending", "", false),
                (9, "unknown", "", true),
            ]
        );
    }
}
