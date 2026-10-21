//! The offline truth of the pipeline (workspace ADR 0010): the `.sql` files are the
//! schema sources, the parsed [`Schema`] is what every consumer agrees on,
//! and a migration is the offline diff between two schemas. Nothing in here
//! needs a PostgreSQL server; the database only verifies what these
//! functions already computed.

use std::fs;
use std::path::{Path, PathBuf};

use crate::ddl::{self, Rename};
use crate::generate::Options;
use crate::ir::Schema;
use crate::parse::{self, SourceFile};
use crate::{Error, naming};

/// Everything the `.sql` files under `options.input` declare, without
/// generating any Rust. Only `Options::input` is read.
pub fn schema(options: &Options) -> Result<Schema, Error> {
    let sources = read_sources(&options.input)?;
    Ok(parse::parse(&sources)?.schema)
}

/// The migration `old -> new` computes to, with `renames` applied instead of
/// drop-and-add.
pub fn migration(old: &Schema, new: &Schema, renames: &[Rename]) -> Result<MigrationPair, Error> {
    let ops = ddl::diff(old, new, renames).map_err(Error::new)?;
    let (up, down) = ddl::scripts(&ops);
    Ok(MigrationPair {
        steps: ops.iter().map(|op| first_line(&op.up).to_owned()).collect(),
        destructive: ops.iter().filter_map(|op| op.destructive.clone()).collect(),
        up,
        down,
    })
}

/// One schema change: the SQL pair to save as a migration, and what guards
/// against surprise data loss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPair {
    /// The forward SQL; empty when the schemas already match.
    pub up: String,
    /// The undo SQL; empty when the change can't be reverted automatically.
    pub down: String,
    /// What each step would destroy, one reason per destructive operation.
    /// A run needs `allow_destructive` (or renames) while this is not empty.
    pub destructive: Vec<String>,
    /// One preview line per operation, for messages.
    pub steps: Vec<String>,
}

impl MigrationPair {
    /// Whether the schemas are equal: no operations at all.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

/// A migration on disk: `migrations/<version>_<name>.up.sql`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationFile {
    /// The number in `0001_init`.
    pub version: i64,
    /// The name in `0001_init`.
    pub name: String,
    /// The whole stem, `0001_init`.
    pub stem: String,
    /// The forward SQL.
    pub up: String,
    /// Whether a `.down.sql` sits beside it.
    pub reversible: bool,
}

/// The `migrations/` directory of a previously generated crate, in version
/// order. A missing directory means no migrations yet.
pub fn existing_migrations(dir: &Path) -> Result<Vec<MigrationFile>, Error> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = file.strip_suffix(".up.sql") else {
            continue;
        };
        let Some((version, name)) = stem.split_once('_') else {
            continue;
        };
        let Ok(version) = version.parse::<i64>() else {
            continue;
        };
        let up =
            fs::read_to_string(entry.path()).map_err(|e| Error::new(format!("{file}: {e}")))?;
        out.push(MigrationFile {
            version,
            name: name.to_owned(),
            stem: stem.to_owned(),
            up,
            reversible: dir.join(format!("{stem}.down.sql")).exists(),
        });
    }
    out.sort_by_key(|m| m.version);
    Ok(out)
}

/// The `.sql` files under `dir`, sorted by path.
pub(crate) fn read_sources(dir: &Path) -> Result<Vec<SourceFile>, Error> {
    let entries = fs::read_dir(dir).map_err(|e| Error::new(format!("{}: {e}", dir.display())))?;
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sql"))
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err(Error::new(format!("no .sql files in {}", dir.display())));
    }
    paths
        .into_iter()
        .map(|path| {
            let text = fs::read_to_string(&path)
                .map_err(|e| Error::new(format!("{}: {e}", path.display())))?;
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let module = if stem.starts_with('_') {
                format!("_{}", naming::snake_ident(&stem))
            } else {
                naming::snake_ident(&stem)
            };
            // Relative to the input directory's parent, so the generated
            // files don't depend on where the command runs.
            let shown = match (dir.file_name(), path.file_name()) {
                (Some(d), Some(f)) => format!("{}/{}", d.to_string_lossy(), f.to_string_lossy()),
                _ => path.display().to_string(),
            };
            Ok(SourceFile {
                path: shown,
                module,
                text,
            })
        })
        .collect()
}

fn first_line(sql: &str) -> &str {
    sql.lines().next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Column, Table};

    fn column(name: &str) -> Column {
        Column {
            name: name.to_owned(),
            sql_type: "TEXT".to_owned(),
            nullable: false,
            default: None,
            generated: None,
            identity: None,
            tenant: false,
            doc: Vec::new(),
        }
    }

    fn table(name: &str, columns: &[&str]) -> Table {
        Table {
            name: name.to_owned(),
            module: name.to_owned(),
            doc: Vec::new(),
            columns: columns.iter().map(|c| column(c)).collect(),
            primary_key: Vec::new(),
            uniques: Vec::new(),
            foreign_keys: Vec::new(),
            checks: Vec::new(),
        }
    }

    fn schema(tables: &[Table]) -> Schema {
        Schema {
            enums: Vec::new(),
            tables: tables.to_vec(),
            indexes: Vec::new(),
            extras: Vec::new(),
        }
    }

    #[test]
    fn identical_schemas_stage_nothing() {
        let schema = schema(&[table("users", &["id"])]);
        let pair = migration(&schema, &schema, &[]).expect("diff succeeds");
        assert!(pair.is_empty());
        assert_eq!(pair.up, "");
        assert_eq!(pair.down, "");
        assert!(pair.destructive.is_empty());
    }

    #[test]
    fn a_new_table_stages_its_create_and_its_drop() {
        let new = schema(&[table("users", &["id"])]);
        let pair = migration(&Schema::default(), &new, &[]).expect("diff succeeds");
        assert!(pair.up.contains("CREATE TABLE"));
        assert!(pair.down.contains("DROP TABLE"));
        assert!(pair.destructive.is_empty());
        assert_eq!(pair.steps.len(), 1);
    }

    #[test]
    fn a_dropped_column_stages_destructively() {
        let old = schema(&[table("users", &["id", "email"])]);
        let new = schema(&[table("users", &["id"])]);
        let pair = migration(&old, &new, &[]).expect("diff succeeds");
        assert!(!pair.destructive.is_empty());
        assert!(pair.up.contains("DROP COLUMN"));
    }

    #[test]
    fn existing_migrations_read_versions_names_and_downs() {
        let root = std::env::temp_dir().join(format!("rok-data-truth-test-{}", line!()));
        let migrations = root.join("migrations");
        fs::create_dir_all(&migrations).expect("create migrations dir");
        fs::write(migrations.join("0001_init.up.sql"), "CREATE TABLE users;\n").expect("write");
        fs::write(
            migrations.join("0002_add_emails.up.sql"),
            "ALTER TABLE\n  users\n  ADD;\n",
        )
        .expect("write");
        fs::write(
            migrations.join("0002_add_emails.down.sql"),
            "ALTER TABLE users\n  DROP;\n",
        )
        .expect("write");
        fs::write(migrations.join("notes.txt"), "not a migration").expect("write");

        let found = existing_migrations(&migrations).expect("read migrations");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].version, 1);
        assert_eq!(found[0].name, "init");
        assert_eq!(found[0].stem, "0001_init");
        assert_eq!(found[0].up, "CREATE TABLE users;\n");
        assert!(!found[0].reversible);
        assert_eq!(found[1].version, 2);
        assert_eq!(found[1].name, "add_emails");
        assert!(found[1].reversible);
        assert_eq!(found[1].up, "ALTER TABLE\n  users\n  ADD;\n");

        assert!(
            existing_migrations(&root.join("missing"))
                .expect("missing dir")
                .is_empty()
        );

        fs::remove_dir_all(&root).ok();
    }
}
