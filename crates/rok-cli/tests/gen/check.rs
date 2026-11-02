//! `rok gen check`: the CI mode, which writes nothing and fails on drift,
//! of the generated code and of the staged history.

use super::*;

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

/// The CI mode guards the staged history too: a truth that moved on since
/// the last staged pair is drift, and the check writes nothing.
///
/// The un-staged truth change is a finding of its own: the prose names
/// the migration the truth moved on from and the version the pair would
/// take, and no file that does not exist is reported among the missing
/// ones.
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
    assert!(out.contains("the truth changed since `0001_init`"), "{out}");
    assert!(out.contains("rok gen --migration <name>"), "{out}");
    assert!(
        out.contains("the check plans `0002_pending`"),
        "the finding names the version the pair would take: {out}"
    );
    assert!(
        !out.contains("missing migrations/0002_pending.up.sql"),
        "the check names no file that does not exist: {out}"
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

    // The same finding, as JSON: its own field, and no missing file.
    let output = run_in(dir.path(), &["gen", "check", "--json"])?;
    assert_eq!(output.status.code(), Some(1));
    let json_out = stdout(&output);
    let report: serde_json::Value =
        serde_json::from_str(json_out.trim()).unwrap_or_else(|e| panic!("{e}: {json_out}"));
    let pending = &report["pending_migration"];
    assert_eq!(pending["version"], serde_json::json!(2));
    assert_eq!(pending["name"], serde_json::json!("pending"));
    assert_eq!(pending["since"], serde_json::json!("0001_init"));
    assert!(
        report["missing"]
            .as_array()
            .expect("the missing files")
            .iter()
            .all(|path| !path.as_str().expect("a path").contains("0002_pending")),
        "the pending pair is not among the missing files: {json_out}"
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
