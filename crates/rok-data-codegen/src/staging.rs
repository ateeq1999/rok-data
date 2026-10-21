//! Staging a migration pair from the truth: the narrow half of the
//! whole-crate generator that the unified front door (`rok gen --migration
//! NAME`) needs — the diff, the guard rails and the two files, without a
//! crate.
//!
//! The project's SQL is split (workspace ADR 0004's amendment) into two
//! directories:
//!
//! - the **truth dir**: plain `*.sql` files describing the wanted schema, the
//!   files the generator reads (and rewrites by hand); it is the project's
//!   single source of truth (workspace ADR 0010);
//! - the **history dir**: the staged migration pairs, `NNNN_name.up.sql` and
//!   the optional `NNNN_name.down.sql`, plus the *snapshot* file they are
//!   diffed against.
//!
//! A migration is the difference between the truth and the state of the last
//! staged migration. That state is the snapshot file, `.rok-schema.json` in
//! the **history** dir: a hidden file, and not a `*.sql` file, so neither
//! the truth's reader nor the runtime's migration loader
//! (`rok migrate`) takes it for one of its own. It is not the engine's
//! registry (`src/db/schema.json`): that one holds the schema as it is
//! *now*, this one holds the schema as of the *last migration*.
//!
//! The two directories default to one — `migrations` for both, the layout
//! `rok init` seeded before the split. The front door keeps such a project
//! staging as before; the loader refuses its truth files, so `rok migrate`
//! reads only the divided layout's `migrations/`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::ddl::Rename;
use crate::ir::Schema;
use crate::naming;
use crate::parse::{self, SourceFile};
use crate::truth;

/// The snapshot file's name inside the history dir.
const SNAPSHOT: &str = ".rok-schema.json";

/// The dir both the truth and the history live in when nothing separates
/// them: the layout `rok init` seeded before the split, and the layout the
/// settings default to so such a project keeps staging as before.
const DEFAULT_DIRS: &str = "migrations";

/// The snapshot file: the schema as of the last staged migration.
///
/// It sits in the history dir, next to the migration pairs, and is a hidden
/// file that is not a `*.sql` file, so neither the truth's reader nor the
/// runtime's migration loader takes it for one of their own.
pub fn snapshot_path(history_dir: &Path) -> PathBuf {
    history_dir.join(SNAPSHOT)
}

/// The truth files of a schema dir: its plain `*.sql` files, without the
/// staged migration pairs (the history), sorted by name.
///
/// The pairs are the history, not the truth: the truth is what the schema
/// dir describes *now*, and the pairs are how it got there. Reading both
/// would apply every change twice.
pub fn truth_files(schema_dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let entries = match fs::read_dir(schema_dir) {
        Ok(entries) => entries,
        // A schema dir that is not there yet has no truth (and no history).
        Err(_) => return Ok(Vec::new()),
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_truth_file(path))
        .collect();
    files.sort();
    Ok(files)
}

/// Whether a file is one of the truth files: a `*.sql` file that is not a
/// staged migration pair.
fn is_truth_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    name.ends_with(".sql") && !name.ends_with(".up.sql") && !name.ends_with(".down.sql")
}

/// What to stage, and how.
#[derive(Debug, Clone)]
pub struct StageOptions {
    /// The truth dir: the `*.sql` files describing the wanted schema.
    pub schema: PathBuf,
    /// The history dir: the staged migration pairs and the snapshot.
    pub history: PathBuf,
    /// The migration's name (`add_posts`).
    pub name: String,
    /// The renames to record instead of a drop-and-add.
    pub renames: Vec<Rename>,
    /// Allow the steps that lose data or fail on existing rows.
    pub allow_destructive: bool,
    /// Plan the run; write nothing.
    pub dry_run: bool,
}

impl Default for StageOptions {
    fn default() -> Self {
        Self {
            schema: PathBuf::from(DEFAULT_DIRS),
            history: PathBuf::from(DEFAULT_DIRS),
            name: String::new(),
            renames: Vec::new(),
            allow_destructive: false,
            dry_run: false,
        }
    }
}

/// The staged migration: the pair of files and what it does.
#[derive(Debug, Clone)]
pub struct Staged {
    /// The version: `1` for the first migration of a directory.
    pub version: i64,
    /// The migration's name.
    pub name: String,
    /// The `NNNN_name.up.sql` file.
    pub up: PathBuf,
    /// The `NNNN_name.down.sql` file, when the steps can be undone.
    pub down: Option<PathBuf>,
    /// One line per step, for the report.
    pub steps: Vec<String>,
    /// Whether the pair can be undone automatically.
    pub reversible: bool,
}

/// Stages the migration: the difference between the truth and the state of
/// the last staged migration, written as a `NNNN_name.up.sql` file and — when
/// the steps can be undone — a `NNNN_name.down.sql` file beside it, into the
/// **history** dir. The snapshot the next staging diffs against sits there
/// too; the truth dir is read only.
///
/// Nothing is overwritten: an existing migration file is an error. The
/// snapshot is updated to the truth, so the next staging diffs against what
/// this one staged.
pub fn stage(options: &StageOptions) -> Result<Staged, Error> {
    let files = truth_files(&options.schema)?;
    if files.is_empty() {
        // A divided truth dir never holds pairs; the one-dir layout does,
        // so the message says why the pairs do not count as truth.
        let besides = if options.schema == options.history {
            " besides its staged migrations"
        } else {
            ""
        };
        return Err(Error::new(format!(
            "{} holds no schema file{besides}: there is nothing to migrate",
            options.schema.display()
        )));
    }
    let sources = read_sources(&files)?;
    let parsed = parse::parse(&sources)?;
    let previous = read_snapshot(&options.history)?;
    let pair = truth::migration(&previous, &parsed.schema, &options.renames)?;
    if pair.is_empty() {
        return Err(Error::nothing_to_migrate());
    }
    let name = naming::snake_ident(&options.name);
    if !pair.destructive.is_empty() && !options.allow_destructive {
        return Err(Error::new(format!(
            "migration `{name}` would:\n  - {}\nre-run with --allow-destructive if that is intended, or use --rename for renames",
            pair.destructive.join("\n  - ")
        )));
    }
    let version = next_version(&options.history)?;
    let stem = format!("{version:04}_{name}");
    let up = options.history.join(format!("{stem}.up.sql"));
    let down = options.history.join(format!("{stem}.down.sql"));
    if up.exists() {
        return Err(Error::new(format!(
            "{} already exists; migrations are never overwritten",
            up.display()
        )));
    }
    let reversible = !pair.down.is_empty();
    if reversible && down.exists() {
        return Err(Error::new(format!(
            "{} already exists; migrations are never overwritten",
            down.display()
        )));
    }
    if !options.dry_run {
        fs::create_dir_all(&options.history)
            .map_err(|e| Error::new(format!("{}: {e}", options.history.display())))?;
        write(&up, &pair.up)?;
        if reversible {
            write(&down, &pair.down)?;
        }
        write_snapshot(&options.history, &parsed.schema)?;
    }
    Ok(Staged {
        version,
        name,
        up,
        down: reversible.then_some(down),
        steps: pair.steps,
        reversible,
    })
}

/// The next free version in the history dir: one above the highest version
/// of the migrations there, so the staged and the hand-made ones
/// (`rok migrate create`) share one numbering. `1` for a directory without
/// migrations.
///
/// The truth files are not migrations, so they hold no version: the first
/// staged migration of a project is `0001_<name>.up.sql`.
fn next_version(dir: &Path) -> Result<i64, Error> {
    let migrations = truth::existing_migrations(dir)?;
    Ok(migrations
        .last()
        .map_or(1, |migration| migration.version + 1))
}

/// The truth files as the parser's sources.
fn read_sources(files: &[PathBuf]) -> Result<Vec<SourceFile>, Error> {
    files
        .iter()
        .map(|path| {
            let text = fs::read_to_string(path)
                .map_err(|e| Error::new(format!("{}: {e}", path.display())))?;
            let stem = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(SourceFile {
                path: path.to_string_lossy().into_owned(),
                module: naming::snake_ident(&stem),
                text,
            })
        })
        .collect()
}

/// The schema as of the last staged migration, or an empty schema when the
/// history dir has no snapshot yet (nothing was staged).
fn read_snapshot(dir: &Path) -> Result<Schema, Error> {
    let path = snapshot_path(dir);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return Ok(Schema::default()),
    };
    serde_json::from_str(&text).map_err(|e| Error::new(format!("{}: {e}", path.display())))
}

/// Records the truth as the state of the last staged migration.
fn write_snapshot(dir: &Path, schema: &Schema) -> Result<(), Error> {
    let path = snapshot_path(dir);
    let text = crate::generate::json(schema)?;
    fs::write(&path, text).map_err(|e| Error::new(format!("{}: {e}", path.display())))
}

/// Writes a migration file.
fn write(path: &Path, sql: &str) -> Result<(), Error> {
    fs::write(path, sql).map_err(|e| Error::new(format!("{}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory under the temp dir, emptied before the test uses it.
    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rok-data-staging-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the schema dir");
        dir
    }

    fn options(dir: &Path) -> StageOptions {
        StageOptions {
            schema: dir.to_path_buf(),
            history: dir.to_path_buf(),
            name: "add_posts".to_owned(),
            ..StageOptions::default()
        }
    }

    /// The error of a failed staging, or an empty string when it succeeded.
    fn message(result: Result<Staged, Error>) -> String {
        match result {
            Ok(_) => String::new(),
            Err(error) => error.to_string(),
        }
    }

    #[test]
    fn the_truth_files_exclude_the_staged_pairs() {
        let dir = dir("truth-files");
        fs::write(dir.join("0001_init.sql"), "CREATE TABLE users (id INT);").expect("write");
        fs::write(
            dir.join("0002_add.up.sql"),
            "ALTER TABLE users ADD COLUMN a INT;",
        )
        .expect("write");
        fs::write(
            dir.join("0002_add.down.sql"),
            "ALTER TABLE users DROP COLUMN a;",
        )
        .expect("write");
        fs::write(dir.join(".rok-schema.json"), "{}").expect("write");

        let files = truth_files(&dir).expect("read the truth files");
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("0001_init.sql"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn staging_writes_the_pair_and_records_the_snapshot() {
        let dir = dir("stage-pair");
        fs::write(
            dir.join("0001_init.sql"),
            "CREATE TABLE users (id BIGINT PRIMARY KEY, email TEXT NOT NULL);",
        )
        .expect("write");

        let staged = stage(&options(&dir)).expect("stage the migration");
        assert_eq!(staged.version, 1, "the truth file holds no version");
        assert_eq!(staged.name, "add_posts");
        assert!(staged.reversible);
        assert!(staged.up.ends_with("0001_add_posts.up.sql"));
        assert!(
            staged
                .down
                .as_ref()
                .is_some_and(|d| d.ends_with("0001_add_posts.down.sql"))
        );

        let up = fs::read_to_string(&staged.up).expect("read the up file");
        assert!(up.contains("CREATE TABLE \"users\""), "{up}");
        let down = fs::read_to_string(staged.down.as_ref().expect("the down file")).expect("read");
        assert!(down.contains("DROP TABLE \"users\""), "{down}");
        assert!(snapshot_path(&dir).is_file(), "the snapshot is recorded");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn staging_nothing_changed_is_an_error() {
        let dir = dir("stage-unchanged");
        fs::write(dir.join("0001_init.sql"), "CREATE TABLE users (id INT);").expect("write");
        stage(&options(&dir)).expect("the first migration stages");
        let again = message(stage(&options(&dir)));
        assert!(again.contains("hasn't changed"), "{again}");
        fs::remove_dir_all(&dir).ok();
    }

    /// The nothing-to-migrate stop is a kind, not a text: the CLI branches
    /// on [`Error::is_nothing_to_migrate`] and the message stays exactly
    /// the wording the CLIs report, so this pin has both.
    #[test]
    fn the_nothing_to_migrate_stop_keeps_its_message_and_is_a_kind() {
        let dir = dir("stage-unchanged-stop");
        fs::write(dir.join("0001_init.sql"), "CREATE TABLE users (id INT);").expect("write");
        stage(&options(&dir)).expect("the first migration stages");

        let stop = stage(&options(&dir)).expect_err("the unchanged truth has no pair");
        assert_eq!(
            stop.to_string(),
            "the schema hasn't changed since the last migration; nothing to migrate"
        );
        assert!(stop.is_nothing_to_migrate());
        assert!(!Error::new("something else failed").is_nothing_to_migrate());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_destructive_step_needs_the_allowance() {
        let dir = dir("stage-destructive");
        fs::write(
            dir.join("0001_init.sql"),
            "CREATE TABLE users (id INT, email TEXT);",
        )
        .expect("write");
        stage(&options(&dir)).expect("the first migration stages");
        fs::write(dir.join("0001_init.sql"), "CREATE TABLE users (id INT);").expect("rewrite");

        let refused = message(stage(&options(&dir)));
        assert!(refused.contains("allow-destructive"), "{refused}");
        let mut allowed = options(&dir);
        allowed.allow_destructive = true;
        let staged = stage(&allowed).expect("the destructive step is allowed");
        assert!(
            fs::read_to_string(&staged.up)
                .expect("read")
                .contains("DROP COLUMN"),
            "the step is staged"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_dry_run_writes_nothing() {
        let dir = dir("stage-dry-run");
        fs::write(dir.join("0001_init.sql"), "CREATE TABLE users (id INT);").expect("write");
        let mut options = options(&dir);
        options.dry_run = true;
        let staged = stage(&options).expect("the dry run plans the migration");
        assert!(!staged.up.exists(), "the dry run wrote the up file");
        assert!(
            !snapshot_path(&dir).exists(),
            "the dry run wrote the snapshot"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_existing_migration_is_never_overwritten() {
        let dir = dir("stage-existing");
        fs::write(dir.join("0001_init.sql"), "CREATE TABLE users (id INT);").expect("write");
        fs::write(dir.join("0002_add_posts.up.sql"), "-- mine\n").expect("write");
        let staged = stage(&options(&dir)).expect("the migration stages");
        assert!(
            staged.up.ends_with("0003_add_posts.up.sql"),
            "the next version is free: {:?}",
            staged.up
        );
        assert_eq!(
            fs::read_to_string(dir.join("0002_add_posts.up.sql")).expect("read"),
            "-- mine\n",
            "the existing migration is untouched"
        );
        fs::remove_dir_all(&dir).ok();
    }

    /// The split layout: the pairs and the snapshot live in the history
    /// dir, the truth dir keeps its plain files and loses nothing.
    #[test]
    fn staging_writes_the_history_into_its_own_dir() {
        let root = dir("stage-split");
        let truth = root.join("schema");
        let history = root.join("migrations");
        fs::create_dir_all(&truth).expect("create the truth dir");
        fs::write(
            truth.join("0001_init.sql"),
            "CREATE TABLE users (id INT, email TEXT);",
        )
        .expect("write");

        let mut options = options(&truth);
        options.history = history.clone();
        let staged = stage(&options).expect("stage the migration");
        assert_eq!(staged.version, 1);
        assert!(staged.up.starts_with(history.join("0001_add_posts.up.sql")));
        assert!(
            snapshot_path(&history).is_file(),
            "the snapshot is recorded"
        );
        assert!(
            truth_files(&truth)
                .expect("read the truth")
                .iter()
                .all(|file| file.ends_with("0001_init.sql")),
            "the truth dir holds no pairs"
        );

        // The next staging diffs against what this one staged.
        fs::write(truth.join("0002_more.sql"), "CREATE TABLE posts (id INT);").expect("write");
        let again = stage(&options).expect("the second migration stages");
        assert_eq!(again.version, 2);
        assert!(again.up.ends_with("0002_add_posts.up.sql"));
        let up = fs::read_to_string(&again.up).expect("read");
        assert!(up.contains("CREATE TABLE \"posts\""), "{up}");
        assert!(!up.contains("users"), "{up}");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_default_dirs_are_one_dir() {
        let options = StageOptions::default();
        assert_eq!(options.schema, PathBuf::from("migrations"));
        assert_eq!(options.history, PathBuf::from("migrations"));
        assert_eq!(options.schema, options.history);
    }

    /// A dry run in the split layout writes nothing, and neither dir is
    /// created.
    #[test]
    fn a_dry_run_writes_nothing_across_two_dirs() {
        let root = dir("stage-split-dry");
        let truth = root.join("schema");
        fs::create_dir_all(&truth).expect("create the truth dir");
        fs::write(truth.join("0001_init.sql"), "CREATE TABLE users (id INT);").expect("write");

        let options = StageOptions {
            schema: truth.clone(),
            history: root.join("migrations"),
            name: "add_posts".to_owned(),
            dry_run: true,
            ..StageOptions::default()
        };
        let staged = stage(&options).expect("the dry run plans the migration");
        assert!(!staged.up.exists(), "the dry run wrote the up file");
        assert!(
            !snapshot_path(&options.history).exists() && !options.history.exists(),
            "the dry run wrote into the history dir"
        );
        fs::remove_dir_all(&root).ok();
    }
}
