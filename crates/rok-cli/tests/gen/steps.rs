//! A `rok gen` step that fails (an App's data layer or migration, the
//! contract export, a Client): the run ends with the steps done, the one
//! that failed, the ones skipped and the command that finishes the rest.
//! Nothing is rolled back.

use super::*;

/// A monorepo whose App `api` has a schema, whose contract's export
/// command does not exist, and whose `ts` Client is emitted from it.
fn failing_export() -> Result<TempDir> {
    let dir = TempDir::new()?;
    fs::write(
        dir.path().join("rok.toml"),
        "[apps]\napi = \"apps/api\"\n\n[contract]\ndocument = \"openapi.json\"\n\
         export = [\"rok-no-such-exporter\"]\n\n[clients]\nts = \"clients/ts\"\n",
    )?;
    fs::write(
        dir.path().join("openapi.json"),
        r#"{"openapi": "3.1.0", "info": {"title": "api", "version": "1"}, "paths": {}}"#,
    )?;
    let app = dir.path().join("apps/api");
    fs::create_dir_all(app.join("schema"))?;
    fs::write(app.join("rok-data-sqlc.toml"), "schema = \"schema\"\n")?;
    fs::write(app.join("schema/0001_init.sql"), SCHEMA)?;
    Ok(dir)
}

/// The data layer and the migration are done and stay done; the Clients
/// are skipped; `rok gen` (without `--migration`: it is staged) finishes.
#[test]
fn a_failing_export_lists_the_steps() -> Result<()> {
    let dir = failing_export()?;
    let output = run_in(dir.path(), &["gen", "--migration", "init"])?;
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    assert!(
        stderr.contains("the contract export failed: re-exporting the contract"),
        "{stderr}"
    );
    assert!(
        stderr.contains(
            "steps:\n  done     `api`'s data layer\n  done     `api`'s migration `init`\n  \
             failed   the contract export\n  skipped  the `ts` client\n"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains(
            "nothing was rolled back; once the cause is fixed, `rok gen` finishes the rest"
        ),
        "{stderr}"
    );
    let app = dir.path().join("apps/api");
    assert!(app.join("src/db/users/models.rs").is_file());
    assert!(app.join("migrations/0001_init.up.sql").is_file());
    assert!(!dir.path().join("clients/ts").exists());
    Ok(())
}

/// `--json` carries the same steps, as the array's last object.
#[test]
fn json_carries_the_steps() -> Result<()> {
    let dir = failing_export()?;
    let output = run_in(dir.path(), &["gen", "--migration", "init", "--json"])?;
    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout(&output);
    let bodies: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    let summary = bodies
        .as_array()
        .and_then(|bodies| bodies.last())
        .cloned()
        .unwrap_or_default();
    let steps = summary["steps"].as_array().cloned().unwrap_or_default();
    let shape: Vec<(String, String)> = steps
        .iter()
        .map(|step| {
            (
                step["kind"].as_str().unwrap_or_default().to_owned(),
                step["status"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    let pair = |kind: &str, status: &str| (kind.to_owned(), status.to_owned());
    assert_eq!(
        shape,
        vec![
            pair("data_layer", "done"),
            pair("migration", "done"),
            pair("export", "failed"),
            pair("client", "skipped"),
        ],
        "{stdout}"
    );
    assert_eq!(steps[0]["app"], "api", "{stdout}");
    assert_eq!(steps[1]["migration"], "init", "{stdout}");
    assert_eq!(steps[3]["client"], "ts", "{stdout}");
    let error = steps[2]["error"].as_str().unwrap_or_default();
    assert!(error.contains("rok-no-such-exporter"), "{stdout}");
    assert_eq!(summary["finish"], "rok gen", "{stdout}");
    Ok(())
}

/// A run of one step that fails is its error alone: no list.
#[test]
fn a_single_step_failure_lists_nothing() -> Result<()> {
    let dir = TempDir::new()?;
    fs::create_dir_all(dir.path().join("migrations"))?;
    let output = run_in(dir.path(), &["gen"])?;
    assert_eq!(output.status.code(), Some(1));
    assert!(!stderr(&output).contains("steps:"), "{}", stderr(&output));
    Ok(())
}

/// A failed generation keeps the flags of the run in the command that
/// finishes it, `--migration` included: nothing was staged yet.
#[test]
fn the_finishing_command_keeps_the_runs_flags() -> Result<()> {
    let dir = project(
        "CREATE TABLE weird (\n    id UUID PRIMARY KEY,\n    payload \"weird type\" NOT NULL\n);\n",
    )?;
    let output = run_in(dir.path(), &["gen", "--migration", "init", "--force"])?;
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    assert!(
        stderr.contains("steps:\n  failed   the data layer\n  skipped  the migration `init`\n"),
        "{stderr}"
    );
    assert!(
        stderr.contains("`rok gen --migration init --force` finishes the rest"),
        "{stderr}"
    );
    Ok(())
}

/// A monorepo whose contract export is a `cargo` command building an App
/// that no longer compiles: a call site still passes one argument to a
/// function that now takes two.
fn uncompilable_export() -> Result<TempDir> {
    let dir = failing_export()?;
    fs::write(
        dir.path().join("rok.toml"),
        "[apps]\napi = \"apps/api\"\n\n[contract]\ndocument = \"openapi.json\"\n\
         export = [\"cargo\", \"run\", \"--quiet\", \"--manifest-path\", \"exporter/Cargo.toml\", \"--\"]\n\n\
         [clients]\nts = \"clients/ts\"\n",
    )?;
    let exporter = dir.path().join("exporter");
    fs::create_dir_all(exporter.join("src"))?;
    fs::write(
        exporter.join("Cargo.toml"),
        "[package]\nname = \"exporter\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )?;
    fs::write(
        exporter.join("src/main.rs"),
        "fn find_user(id: u32, active: bool) -> u32 {\n    if active { id } else { 0 }\n}\n\n\
         fn main() {\n    println!(\"{}\", find_user(1));\n}\n",
    )?;
    Ok(dir)
}

/// An App that no longer builds says so, with the compiler's first error.
#[test]
fn an_export_that_does_not_compile_says_so() -> Result<()> {
    let dir = uncompilable_export()?;
    let output = run_in(dir.path(), &["gen"])?;
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    assert!(
        stderr.contains(
            "the contract export failed: the App doesn't build after this change: fix the call \
             sites, then run `rok gen` to write the contract and the Clients"
        ),
        "{stderr}"
    );
    let summary = &stderr[stderr
        .find("the contract export failed")
        .unwrap_or_default()..];
    assert!(summary.contains("error[E0061]"), "{stderr}");
    assert!(summary.contains("main.rs:6:20"), "{stderr}");
    assert!(!summary.contains("help: provide the argument"), "{stderr}");
    assert!(
        summary.contains("steps:\n  done     `api`'s data layer\n  failed   the contract export\n"),
        "{stderr}"
    );
    Ok(())
}

/// `--json` carries the same message in the failed step.
#[test]
fn json_carries_an_export_that_does_not_compile() -> Result<()> {
    let dir = uncompilable_export()?;
    let output = run_in(dir.path(), &["gen", "--json"])?;
    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout(&output);
    let bodies: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    let steps = bodies
        .as_array()
        .and_then(|bodies| bodies.last())
        .map(|summary| summary["steps"].clone())
        .unwrap_or_default();
    let export = steps
        .as_array()
        .and_then(|steps| steps.iter().find(|step| step["kind"] == "export"))
        .cloned()
        .unwrap_or_default();
    assert_eq!(export["status"], "failed", "{stdout}");
    let error = export["error"].as_str().unwrap_or_default();
    assert!(
        error.starts_with("the App doesn't build after this change"),
        "{stdout}"
    );
    assert!(error.contains("error[E0061]"), "{stdout}");
    Ok(())
}

/// A `cargo` export that fails without compiling anything keeps the
/// export's own message.
#[test]
fn a_cargo_export_failing_otherwise_keeps_its_message() -> Result<()> {
    let dir = failing_export()?;
    fs::write(
        dir.path().join("rok.toml"),
        "[apps]\napi = \"apps/api\"\n\n[contract]\ndocument = \"openapi.json\"\n\
         export = [\"cargo\", \"rok-no-such-subcommand\"]\n\n[clients]\nts = \"clients/ts\"\n",
    )?;
    let output = run_in(dir.path(), &["gen"])?;
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    assert!(
        stderr.contains("the contract export failed: re-exporting the contract"),
        "{stderr}"
    );
    assert!(!stderr.contains("doesn't build"), "{stderr}");
    Ok(())
}
