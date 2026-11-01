//! `rok gen`'s generate run: the data layer from the schema dir, where it
//! reads and writes, and what fails it.

use super::*;

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
