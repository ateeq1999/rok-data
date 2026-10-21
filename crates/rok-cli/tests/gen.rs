//! `rok gen` / `rok gen check`: the unified front door. It generates the
//! data layer and the API features from the project's schema dir, and
//! stages a migration pair when it is asked to.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use anyhow::Result;
use tempfile::TempDir;

/// A schema with one table.
const SCHEMA: &str =
    "CREATE TABLE users (\n    id UUID PRIMARY KEY,\n    email TEXT NOT NULL\n);\n";

/// A schema with a second table.
const SCHEMA_WITH_POSTS: &str = "CREATE TABLE users (\n    id UUID PRIMARY KEY,\n    email TEXT NOT NULL\n);\n\nCREATE TABLE posts (\n    id UUID PRIMARY KEY,\n    title TEXT NOT NULL\n);\n";

fn rok() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rok"))
}

fn run_in(dir: &Path, args: &[&str]) -> Result<Output> {
    Ok(rok().current_dir(dir).args(args).output()?)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A project whose schema dir holds `schema`.
fn project(schema: &str) -> Result<TempDir> {
    let dir = TempDir::new()?;
    fs::create_dir_all(dir.path().join("migrations"))?;
    fs::write(dir.path().join("migrations/0001_init.sql"), schema)?;
    Ok(dir)
}

/// The migrations of a project's schema dir, by name.
fn migrations(dir: &Path) -> Result<Vec<String>> {
    let mut names: Vec<String> = fs::read_dir(dir.join("migrations"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".sql") && !name.starts_with('.'))
        .collect();
    names.sort();
    Ok(names)
}

#[test]
fn gen_generates_the_data_layer_from_the_schema_dir() -> Result<()> {
    let dir = project(SCHEMA)?;
    let output = run_in(dir.path(), &["gen"])?;
    assert_success(&output);

    assert!(dir.path().join("src/db/users/models.rs").is_file());
    assert!(dir.path().join("src/db/users/read.rs").is_file());
    assert!(dir.path().join("src/db/mod.rs").is_file());
    assert!(dir.path().join("src/db/schema.json").is_file());
    let stdout = stdout(&output);
    assert!(
        stdout.contains("generated src/db/users/models.rs"),
        "{stdout}"
    );
    Ok(())
}

#[test]
fn gen_reads_the_schema_dir_the_flag_names() -> Result<()> {
    let dir = TempDir::new()?;
    fs::create_dir_all(dir.path().join("schema"))?;
    fs::write(dir.path().join("schema/0001_init.sql"), SCHEMA)?;

    let output = run_in(dir.path(), &["gen", "--schema", "schema"])?;
    assert_success(&output);
    assert!(dir.path().join("src/db/users/models.rs").is_file());
    Ok(())
}

#[test]
fn gen_writes_where_the_flag_says() -> Result<()> {
    let dir = project(SCHEMA)?;
    let output = run_in(dir.path(), &["gen", "--output", "src/data"])?;
    assert_success(&output);
    assert!(dir.path().join("src/data/users/models.rs").is_file());
    assert!(!dir.path().join("src/db").exists());
    Ok(())
}

#[test]
fn gen_is_idempotent() -> Result<()> {
    let dir = project(SCHEMA)?;
    assert_success(&run_in(dir.path(), &["gen"])?);
    let models = fs::read_to_string(dir.path().join("src/db/users/models.rs"))?;
    assert_success(&run_in(dir.path(), &["gen"])?);
    assert_eq!(
        fs::read_to_string(dir.path().join("src/db/users/models.rs"))?,
        models
    );
    Ok(())
}

/// The migration pair lands in the schema dir, in the runtime's convention.
#[test]
fn gen_stages_a_migration_pair_in_the_schema_dir() -> Result<()> {
    let dir = project(SCHEMA)?;
    let output = run_in(dir.path(), &["gen", "--migration", "init"])?;
    assert_success(&output);

    assert_eq!(
        migrations(dir.path())?,
        vec![
            "0001_init.down.sql".to_owned(),
            "0001_init.sql".to_owned(),
            "0001_init.up.sql".to_owned(),
        ]
    );
    let up = fs::read_to_string(dir.path().join("migrations/0001_init.up.sql"))?;
    assert!(up.contains("CREATE TABLE"), "{up}");
    let down = fs::read_to_string(dir.path().join("migrations/0001_init.down.sql"))?;
    assert!(down.contains("DROP TABLE"), "{down}");
    Ok(())
}

/// A staged migration is history: the generator reads the truth, so the
/// change is never applied twice.
#[test]
fn a_staged_migration_does_not_change_the_generated_code() -> Result<()> {
    let dir = project(SCHEMA)?;
    assert_success(&run_in(dir.path(), &["gen", "--migration", "init"])?);
    let models = fs::read_to_string(dir.path().join("src/db/users/models.rs"))?;

    assert_success(&run_in(dir.path(), &["gen"])?);
    assert_eq!(
        fs::read_to_string(dir.path().join("src/db/users/models.rs"))?,
        models
    );
    Ok(())
}

/// The second change is the difference against the last staged migration.
#[test]
fn a_second_migration_carries_only_the_change() -> Result<()> {
    let dir = project(SCHEMA)?;
    assert_success(&run_in(dir.path(), &["gen", "--migration", "init"])?);
    fs::write(
        dir.path().join("migrations/0001_init.sql"),
        SCHEMA_WITH_POSTS,
    )?;

    let output = run_in(dir.path(), &["gen", "--migration", "add_posts"])?;
    assert_success(&output);
    let up = fs::read_to_string(dir.path().join("migrations/0002_add_posts.up.sql"))?;
    assert!(up.contains("CREATE TABLE \"posts\""), "{up}");
    assert!(!up.contains("CREATE TABLE \"users\""), "{up}");
    Ok(())
}

/// A step that loses data is refused unless it is allowed.
#[test]
fn a_destructive_step_needs_the_allowance() -> Result<()> {
    let dir = project(SCHEMA)?;
    assert_success(&run_in(dir.path(), &["gen", "--migration", "init"])?);
    fs::write(
        dir.path().join("migrations/0001_init.sql"),
        "CREATE TABLE users (\n    id UUID PRIMARY KEY\n);\n",
    )?;

    let refused = run_in(dir.path(), &["gen", "--migration", "drop_email"])?;
    assert_eq!(refused.status.code(), Some(1));
    let stderr = stderr(&refused);
    assert!(stderr.contains("allow-destructive"), "{stderr}");
    assert!(
        !dir.path()
            .join("migrations/0002_drop_email.up.sql")
            .exists()
    );

    let allowed = run_in(
        dir.path(),
        &["gen", "--migration", "drop_email", "--allow-destructive"],
    )?;
    assert_success(&allowed);
    let up = fs::read_to_string(dir.path().join("migrations/0002_drop_email.up.sql"))?;
    assert!(up.contains("DROP COLUMN"), "{up}");
    Ok(())
}

/// A rename is recorded as a rename, not as a drop and an add.
#[test]
fn a_rename_is_recorded_as_a_rename() -> Result<()> {
    let dir = project(SCHEMA)?;
    assert_success(&run_in(dir.path(), &["gen", "--migration", "init"])?);
    fs::write(
        dir.path().join("migrations/0001_init.sql"),
        "CREATE TABLE users (\n    id UUID PRIMARY KEY,\n    email_address TEXT NOT NULL\n);\n",
    )?;

    let output = run_in(
        dir.path(),
        &[
            "gen",
            "--migration",
            "rename_email",
            "--rename",
            "users.email=email_address",
        ],
    )?;
    assert_success(&output);
    let up = fs::read_to_string(dir.path().join("migrations/0002_rename_email.up.sql"))?;
    assert!(up.contains("RENAME COLUMN"), "{up}");
    assert!(!up.contains("DROP COLUMN"), "{up}");
    Ok(())
}

/// An existing migration is never overwritten.
#[test]
fn an_existing_migration_is_never_overwritten() -> Result<()> {
    let dir = project(SCHEMA)?;
    fs::write(dir.path().join("migrations/0001_init.up.sql"), "-- mine\n")?;

    let output = run_in(dir.path(), &["gen", "--migration", "init"])?;
    assert_success(&output);
    assert_eq!(
        fs::read_to_string(dir.path().join("migrations/0001_init.up.sql"))?,
        "-- mine\n"
    );
    Ok(())
}

/// The CI mode: nothing is written, and the drift fails the run.
#[test]
fn gen_check_passes_when_the_code_is_up_to_date() -> Result<()> {
    let dir = project(SCHEMA)?;
    assert_success(&run_in(dir.path(), &["gen"])?);

    let output = run_in(dir.path(), &["gen", "check"])?;
    assert_success(&output);
    assert!(
        stdout(&output).contains("up to date"),
        "{}",
        stdout(&output)
    );
    Ok(())
}

#[test]
fn gen_check_fails_on_drift() -> Result<()> {
    let dir = project(SCHEMA)?;
    assert_success(&run_in(dir.path(), &["gen"])?);
    fs::write(
        dir.path().join("migrations/0001_init.sql"),
        format!("{SCHEMA}\nCREATE TABLE posts (id UUID PRIMARY KEY);\n"),
    )?;

    let output = run_in(dir.path(), &["gen", "check"])?;
    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout(&output);
    assert!(
        stdout.contains("missing src/db/posts/models.rs"),
        "{stdout}"
    );
    Ok(())
}

/// The CI mode writes nothing.
#[test]
fn gen_check_writes_nothing() -> Result<()> {
    let dir = project(SCHEMA)?;
    let output = run_in(dir.path(), &["gen", "check"])?;
    assert_eq!(output.status.code(), Some(1));
    assert!(!dir.path().join("src/db").exists());
    Ok(())
}

/// The flags of the group and of the leaf: the leaf wins.
#[test]
fn the_leaf_flags_win_over_the_group_flags() -> Result<()> {
    let dir = project(SCHEMA)?;
    let output = run_in(
        dir.path(),
        &[
            "gen",
            "--output",
            "src/group",
            "check",
            "--output",
            "src/leaf",
        ],
    )?;
    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout(&output);
    assert!(stdout.contains("src/leaf"), "{stdout}");
    assert!(!stdout.contains("src/group"), "{stdout}");
    Ok(())
}

/// The staging flags belong to the generate run: given to the group, they
/// are refused; given to the leaf, they are not even flags.
#[test]
fn gen_check_rejects_the_staging_flags() -> Result<()> {
    let dir = project(SCHEMA)?;

    let output = run_in(dir.path(), &["gen", "--migration", "init", "check"])?;
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    assert!(stderr.contains("--migration"), "{stderr}");
    assert!(stderr.contains("rok gen"), "{stderr}");

    let output = run_in(dir.path(), &["gen", "check", "--migration", "init"])?;
    assert_eq!(output.status.code(), Some(2));
    Ok(())
}

/// The machine-readable report: one JSON object on stdout.
#[test]
fn gen_json_reports_the_run() -> Result<()> {
    let dir = project(SCHEMA)?;
    let output = run_in(dir.path(), &["gen", "--json"])?;
    assert_success(&output);

    let stdout = stdout(&output);
    let report: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    let written = report["written"].as_array().expect("the written files");
    assert!(
        written
            .iter()
            .any(|path| path.as_str() == Some("src/db/users/models.rs")),
        "{stdout}"
    );
    assert_eq!(report["drift"], serde_json::json!(false));
    assert_eq!(report["migration"], serde_json::Value::Null);
    Ok(())
}

#[test]
fn gen_json_reports_the_drift_of_a_check() -> Result<()> {
    let dir = project(SCHEMA)?;
    assert_success(&run_in(dir.path(), &["gen"])?);
    fs::write(
        dir.path().join("migrations/0001_init.sql"),
        format!("{SCHEMA}\nCREATE TABLE posts (id UUID PRIMARY KEY);\n"),
    )?;

    let output = run_in(dir.path(), &["gen", "check", "--json"])?;
    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout(&output);
    let report: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    assert_eq!(report["drift"], serde_json::json!(true));
    assert!(
        !report["missing"]
            .as_array()
            .expect("the missing files")
            .is_empty()
    );
    Ok(())
}

#[test]
fn gen_json_reports_the_staged_migration() -> Result<()> {
    let dir = project(SCHEMA)?;
    let output = run_in(dir.path(), &["gen", "--migration", "init", "--json"])?;
    assert_success(&output);

    let stdout = stdout(&output);
    let report: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    let migration = &report["migration"];
    assert_eq!(migration["version"], serde_json::json!(1));
    assert_eq!(migration["name"], serde_json::json!("init"));
    assert_eq!(
        migration["up"],
        serde_json::json!("migrations/0001_init.up.sql")
    );
    Ok(())
}

/// A dry run plans the run and writes nothing.
#[test]
fn gen_dry_run_writes_nothing() -> Result<()> {
    let dir = project(SCHEMA)?;
    let output = run_in(dir.path(), &["gen", "--dry-run", "--migration", "init"])?;
    assert_success(&output);

    let stdout = stdout(&output);
    assert!(
        stdout.contains("would write src/db/users/models.rs"),
        "{stdout}"
    );
    assert!(
        stdout.contains("would stage migrations/0001_init.up.sql"),
        "{stdout}"
    );
    assert!(!dir.path().join("src/db").exists());
    assert_eq!(migrations(dir.path())?, vec!["0001_init.sql".to_owned()]);
    Ok(())
}

/// The settings file: the unified project file, with the generation
/// settings in its `[generate]` section.
#[test]
fn gen_reads_the_unified_project_file() -> Result<()> {
    let dir = project(SCHEMA)?;
    fs::write(
        dir.path().join("rok.toml"),
        "[generate]\nschema = \"migrations/\"\noutput = \"src/data\"\n",
    )?;

    let output = run_in(dir.path(), &["gen"])?;
    assert_success(&output);
    assert!(dir.path().join("src/data/users/models.rs").is_file());
    Ok(())
}

/// The engine's own file stays readable, with its keys at the top level.
#[test]
fn gen_reads_the_engine_config_file() -> Result<()> {
    let dir = project(SCHEMA)?;
    fs::write(
        dir.path().join("rok-data-sqlc.toml"),
        "schema = \"migrations/\"\noutput = \"src/data\"\n",
    )?;

    let output = run_in(dir.path(), &["gen"])?;
    assert_success(&output);
    assert!(dir.path().join("src/data/users/models.rs").is_file());
    Ok(())
}

/// The flags beat the settings file (ADR 0011).
#[test]
fn the_flags_beat_the_settings_file() -> Result<()> {
    let dir = project(SCHEMA)?;
    fs::write(
        dir.path().join("rok.toml"),
        "[generate]\nschema = \"migrations/\"\noutput = \"src/data\"\n",
    )?;

    let output = run_in(dir.path(), &["gen", "--output", "src/flag"])?;
    assert_success(&output);
    assert!(dir.path().join("src/flag/users/models.rs").is_file());
    assert!(!dir.path().join("src/data").exists());
    Ok(())
}

/// The config file the flag names is the one that is read.
#[test]
fn the_config_flag_names_the_file() -> Result<()> {
    let dir = project(SCHEMA)?;
    fs::write(
        dir.path().join("other.toml"),
        "schema = \"migrations/\"\noutput = \"src/other\"\n",
    )?;

    let output = run_in(dir.path(), &["gen", "--config", "other.toml"])?;
    assert_success(&output);
    assert!(dir.path().join("src/other/users/models.rs").is_file());
    Ok(())
}

/// The legacy commands keep working: the whole-crate generator and the
/// engine's own grammar.
#[test]
fn the_legacy_commands_keep_working() -> Result<()> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("schema.sql"), SCHEMA)?;

    let generate = run_in(
        dir.path(),
        &["generate", "--input", ".", "--output", "db-gen"],
    )?;
    assert_success(&generate);
    assert!(dir.path().join("db-gen/Cargo.toml").is_file());

    let sqlc = run_in(
        dir.path(),
        &[
            "sqlc",
            "generate",
            "--schema",
            "schema.sql",
            "--output",
            "src/db",
        ],
    )?;
    assert_success(&sqlc);
    assert!(dir.path().join("src/db/users/models.rs").is_file());
    Ok(())
}

/// A table the generator cannot handle is a warning; `--strict` makes it a
/// failure.
#[test]
fn a_warning_fails_a_strict_run() -> Result<()> {
    let dir = project(
        "CREATE TABLE users (\n    id UUID PRIMARY KEY\n);\n\
         CREATE TABLE weird (\n    id UUID PRIMARY KEY,\n    payload \"weird type\" NOT NULL\n);\n",
    )?;

    let lenient = run_in(dir.path(), &["gen"])?;
    assert_success(&lenient);
    assert!(
        stderr(&lenient).contains("warning: table `weird` skipped"),
        "{}",
        stderr(&lenient)
    );

    let strict = run_in(dir.path(), &["gen", "--strict"])?;
    assert_eq!(strict.status.code(), Some(1));
    assert!(stderr(&strict).contains("--strict"), "{}", stderr(&strict));
    Ok(())
}

/// A schema dir without a `.sql` file says so.
#[test]
fn an_empty_schema_dir_fails() -> Result<()> {
    let dir = TempDir::new()?;
    fs::create_dir_all(dir.path().join("migrations"))?;

    let output = run_in(dir.path(), &["gen"])?;
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("no `*.sql` migrations"),
        "{}",
        stderr(&output)
    );
    Ok(())
}

/// The `--migration` needs a schema directory.
#[test]
fn a_migration_needs_a_schema_directory() -> Result<()> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("schema.sql"), SCHEMA)?;

    let output = run_in(
        dir.path(),
        &["gen", "--schema", "schema.sql", "--migration", "init"],
    )?;
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("schema directory"),
        "{}",
        stderr(&output)
    );
    Ok(())
}

/// The advice names a way that works: the runtime's loader reads the
/// history dir's pairs, so the staged pair is applied by `rok migrate up`.
#[test]
fn the_migration_advice_names_the_staged_pair() -> Result<()> {
    let dir = project(SCHEMA)?;
    let output = run_in(dir.path(), &["gen", "--migration", "init"])?;
    assert_success(&output);

    let stdout = stdout(&output);
    assert!(stdout.contains("migrations/0001_init.up.sql"), "{stdout}");
    assert!(stdout.contains("rok migrate up"), "{stdout}");
    assert!(!stdout.contains("psql"), "{stdout}");
    Ok(())
}

/// A history dir moved off its default is named in the advice.
#[test]
fn the_migration_advice_names_a_moved_history_dir() -> Result<()> {
    let dir = TempDir::new()?;
    fs::create_dir_all(dir.path().join("schema"))?;
    fs::write(dir.path().join("schema/0001_init.sql"), SCHEMA)?;

    let output = run_in(
        dir.path(),
        &[
            "gen",
            "--schema",
            "schema",
            "--migrations-dir",
            "history",
            "--migration",
            "init",
        ],
    )?;
    assert_success(&output);
    assert!(
        dir.path().join("history/0001_init.up.sql").is_file(),
        "the pair lands in the history dir"
    );
    assert!(
        !dir.path().join("schema/0001_init.up.sql").exists(),
        "the truth dir never holds pairs"
    );
    let stdout = stdout(&output);
    assert!(stdout.contains("history/0001_init.up.sql"), "{stdout}");
    assert!(stdout.contains("--dir history"), "{stdout}");
    Ok(())
}

/// The CI mode guards the staged history too: a truth that moved on since
/// the last staged pair is drift, and the check writes nothing.
#[test]
fn gen_check_fails_on_a_pending_migration() -> Result<()> {
    let dir = project(SCHEMA)?;
    assert_success(&run_in(dir.path(), &["gen", "--migration", "init"])?);
    fs::write(
        dir.path().join("migrations/0001_init.sql"),
        SCHEMA_WITH_POSTS,
    )?;
    assert_success(&run_in(dir.path(), &["gen"])?);

    let output = run_in(dir.path(), &["gen", "check"])?;
    assert_eq!(output.status.code(), Some(1));
    let out = stdout(&output);
    assert!(
        out.contains("missing migrations/0002_pending.up.sql"),
        "{out}"
    );
    let err = stderr(&output);
    assert!(
        err.contains("not staged as a migration"),
        "the error names the staging: {err}"
    );
    assert!(
        !dir.path().join("migrations/0002_pending.up.sql").exists()
            && !dir.path().join("migrations/0002_pending.down.sql").exists(),
        "the check writes nothing"
    );

    // Staging for real unsticks the check.
    assert_success(&run_in(dir.path(), &["gen", "--migration", "add_posts"])?);
    let output = run_in(dir.path(), &["gen", "check"])?;
    assert_success(&output);
    assert!(
        stdout(&output).contains("up to date"),
        "{}",
        stdout(&output)
    );
    Ok(())
}

/// An up-to-date truth with staged pairs is not drift.
#[test]
fn gen_check_passes_when_the_history_is_staged() -> Result<()> {
    let dir = project(SCHEMA)?;
    assert_success(&run_in(dir.path(), &["gen", "--migration", "init"])?);
    assert_success(&run_in(dir.path(), &["gen"])?);

    let output = run_in(dir.path(), &["gen", "check"])?;
    assert_success(&output);
    assert!(
        stdout(&output).contains("up to date"),
        "{}",
        stdout(&output)
    );
    Ok(())
}

/// The `--api-framework` flag picks the API backend, and beats the file.
#[test]
fn the_api_framework_flag_beats_the_file() -> Result<()> {
    let dir = project(SCHEMA)?;
    fs::write(
        dir.path().join("rok.toml"),
        "[generate]\nschema = \"migrations/\"\nderive = [\"serde\", \"validate\"]\n\
         extractors = [\"actix_web\", \"axum\"]\n\n[api]\noutput = \"src/features\"\n\
         framework = \"actix_web\"\n",
    )?;

    let output = run_in(dir.path(), &["gen", "--api-framework", "axum"])?;
    assert_success(&output);
    let routes = fs::read_to_string(dir.path().join("src/features/users/routes.rs"))?;
    assert!(routes.contains("axum::Router"), "{routes}");
    Ok(())
}

/// The flag needs the API features it selects a backend for.
#[test]
fn the_api_framework_flag_needs_the_api_section() -> Result<()> {
    let dir = project(SCHEMA)?;

    let output = run_in(dir.path(), &["gen", "--api-framework", "axum"])?;
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    assert!(stderr.contains("--api-framework"), "{stderr}");
    assert!(stderr.contains("[api]"), "{stderr}");
    Ok(())
}

/// The unified project's `[generate]` section belongs to `rok gen`: the
/// legacy generator says so in one line instead of an unknown-key error.
#[test]
fn the_legacy_generator_names_the_front_doors_section() -> Result<()> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("schema.sql"), SCHEMA)?;
    fs::write(
        dir.path().join("rok.toml"),
        "[generate]\nschema = \"schema.sql\"\nderive = [\"serde\"]\n",
    )?;

    let output = run_in(
        dir.path(),
        &["generate", "--input", ".", "--output", "db-gen"],
    )?;
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    assert!(stderr.contains("belongs to `rok gen`"), "{stderr}");
    assert!(stderr.contains("[gen]"), "{stderr}");
    Ok(())
}
