//! `rok gen --json`: the machine-readable report, one JSON object on
//! stdout.

use super::*;

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
