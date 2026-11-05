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

/// A table the generator cannot handle fails the run; `--lenient` skips it
/// with a warning, and `--strict` (the default) is still accepted.
#[test]
fn an_unsupported_table_fails_unless_lenient() -> Result<()> {
    let dir = project(
        "CREATE TABLE users (\n    id UUID PRIMARY KEY\n);\n\
         CREATE TABLE weird (\n    id UUID PRIMARY KEY,\n    payload \"weird type\" NOT NULL\n);\n",
    )?;

    for args in [&["gen"][..], &["gen", "--strict"][..]] {
        let strict = run_in(dir.path(), args)?;
        assert_eq!(strict.status.code(), Some(1), "{args:?}");
        let stderr = stderr(&strict);
        assert!(stderr.contains("table `weird`"), "{stderr}");
        assert!(!stderr.contains("skipped"), "{stderr}");
        assert!(stderr.contains("--lenient"), "{stderr}");
    }
    assert!(
        !dir.path().join("src/db").exists(),
        "a failure writes nothing"
    );

    let lenient = run_in(dir.path(), &["gen", "--lenient"])?;
    assert_success(&lenient);
    assert!(
        stderr(&lenient).contains("warning: table `weird` skipped"),
        "{}",
        stderr(&lenient)
    );
    Ok(())
}

/// The schema with an expiry the `remaining` query subtracts from.
const SESSIONS: &str =
    "CREATE TABLE sessions (\n    id UUID PRIMARY KEY,\n    expires_at TIMESTAMPTZ NOT NULL\n);\n";

/// A query whose result column the engine can't type: it does not type
/// date arithmetic.
const EXPIRED_QUERY: &str = "-- name: SessionExpiry :one\nSELECT id, expires_at - now() AS remaining FROM sessions WHERE id = $1;\n";

/// A project with [`SESSIONS`] and the [`EXPIRED_QUERY`] in `queries/`.
fn expiry_project() -> Result<TempDir> {
    let dir = project(SESSIONS)?;
    fs::create_dir_all(dir.path().join("queries"))?;
    fs::write(dir.path().join("queries/sessions.sql"), EXPIRED_QUERY)?;
    Ok(dir)
}

/// A query whose result column can't be typed fails `rok gen` (and
/// `rok gen check`), naming the file, the query, the expression and the
/// `-- column:` override that fixes it.
#[test]
fn an_untypeable_query_fails_naming_the_fix() -> Result<()> {
    let dir = expiry_project()?;
    for args in [
        &["gen", "--queries", "queries"][..],
        &["gen", "check", "--queries", "queries"][..],
    ] {
        let output = run_in(dir.path(), args)?;
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let stderr = stderr(&output);
        assert!(
            stderr.contains("error: queries/sessions.sql:1: query `SessionExpiry`"),
            "{stderr}"
        );
        assert!(stderr.contains("`expires_at - now()`"), "{stderr}");
        assert!(
            stderr.contains("add `-- column: remaining <RustType>`"),
            "{stderr}"
        );
        assert!(!stderr.contains("skipped"), "{stderr}");
    }
    assert!(
        !dir.path().join("src/db").exists(),
        "a failure writes nothing"
    );

    // The fix the error names generates the query.
    fs::write(
        dir.path().join("queries/sessions.sql"),
        EXPIRED_QUERY.replace(
            ":one\n",
            ":one\n-- column: remaining sqlx::postgres::types::PgInterval\n",
        ),
    )?;
    assert_success(&run_in(dir.path(), &["gen", "--queries", "queries"])?);
    Ok(())
}

/// `--lenient` skips the query with a warning, as `rok gen` did before.
#[test]
fn lenient_skips_an_untypeable_query() -> Result<()> {
    let dir = expiry_project()?;
    let output = run_in(dir.path(), &["gen", "--queries", "queries", "--lenient"])?;
    assert_success(&output);
    let stderr = stderr(&output);
    assert!(
        stderr.contains("warning: query `SessionExpiry` skipped: "),
        "{stderr}"
    );
    assert!(dir.path().join("src/db/sessions/models.rs").is_file());
    Ok(())
}

/// The `--json` body carries the failure: where, which query, and why.
#[test]
fn json_carries_the_untypeable_query() -> Result<()> {
    let dir = expiry_project()?;
    let output = run_in(dir.path(), &["gen", "--queries", "queries", "--json"])?;
    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout(&output);
    let body: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    let failure = &body["failures"][0];
    assert_eq!(failure["kind"], "query", "{stdout}");
    assert_eq!(failure["name"], "SessionExpiry", "{stdout}");
    assert_eq!(failure["path"], "queries/sessions.sql", "{stdout}");
    assert_eq!(failure["line"], 1, "{stdout}");
    let reason = failure["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains("-- column: remaining <RustType>"),
        "{stdout}"
    );
    assert_eq!(body["skipped"], serde_json::json!([]), "{stdout}");
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
