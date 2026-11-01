//! `rok gen --migration`: staging a migration pair in the history dir,
//! and the advice that names it.

use super::*;

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

/// In a monorepo, an App whose settings file leaves `migrations` unset
/// stages its pair into its own `migrations/`, not the root's: the
/// default resolves against the App's directory, as `schema` does.
#[test]
fn gen_stages_an_apps_migration_in_the_apps_history_dir() -> Result<()> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("rok.toml"), "[apps]\napi = \"apps/api\"\n")?;
    let app = dir.path().join("apps/api");
    fs::create_dir_all(app.join("schema"))?;
    fs::write(app.join("rok-data-sqlc.toml"), "schema = \"schema\"\n")?;
    fs::write(app.join("schema/0001_init.sql"), SCHEMA)?;

    let output = run_in(dir.path(), &["gen", "--migration", "init"])?;
    assert_success(&output);

    assert!(
        app.join("migrations/0001_init.up.sql").is_file(),
        "{}",
        stdout(&output)
    );
    assert!(app.join("migrations/0001_init.down.sql").is_file());
    assert!(
        !dir.path().join("migrations").exists(),
        "the pair was staged at the root: {}",
        stdout(&output)
    );
    Ok(())
}
