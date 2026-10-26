//! `rok init [NAME] [--template <NAME>] [--schema <PATH>]`: the root
//! front door's scaffolding command. It writes the application skeleton, or
//! a ready-made template, into a project, and seeds the project's schema
//! directory — every file written once and never rewritten.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::Result;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;

/// The skeleton `init` scaffolds, relative to the project directory.
const SKELETON: &[&str] = &[
    "AGENTS.md",
    "rok-data-sqlc.toml",
    "src/app.rs",
    "src/config.rs",
    "src/error.rs",
    "src/features/mod.rs",
    "src/lib.rs",
    "src/main.rs",
    "src/openapi.rs",
    "src/shared/auth.rs",
    "src/shared/mod.rs",
];

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

/// The step lines after `Next steps:`.
fn next_steps(output: &Output) -> Vec<String> {
    stdout(output)
        .lines()
        .skip_while(|line| *line != "Next steps:")
        .skip(1)
        .map(|line| line.trim_start().to_owned())
        .collect()
}

#[test]
fn init_scaffolds_the_skeleton_into_the_current_directory() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_in(dir.path(), &["init"])?;
    assert_success(&output);

    for file in SKELETON {
        assert!(dir.path().join(file).is_file(), "{file} is not scaffolded");
    }
    assert!(dir.path().join("schema").is_dir());
    assert!(
        !dir.path().join("migrations").is_dir(),
        "the history dir appears with the first staged pair, not before"
    );
    assert!(
        !dir.path().join("src/db").exists(),
        "init generates nothing"
    );

    let stdout = stdout(&output);
    for file in SKELETON {
        assert!(
            stdout.contains(&format!("scaffolded {file}")),
            "{file} not reported:\n{stdout}"
        );
    }
    Ok(())
}

#[test]
fn init_scaffolds_into_the_named_directory() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_in(dir.path(), &["init", "my-app"])?;
    assert_success(&output);

    let project = dir.path().join("my-app");
    assert!(project.join("src/app.rs").is_file());
    assert!(project.join("schema").is_dir());
    // The library crate is named after the directory.
    let main = fs::read_to_string(project.join("src/main.rs"))?;
    assert!(main.contains("\nuse my_app::{app, config};\n"), "{main}");
    Ok(())
}

/// The scaffolded-file policy: a file is written once and never rewritten,
/// whoever owns it afterwards.
#[test]
fn init_never_rewrites_a_file_that_is_already_there() -> Result<()> {
    let dir = TempDir::new()?;
    assert_success(&run_in(dir.path(), &["init"])?);

    let error = dir.path().join("src/error.rs");
    fs::write(&error, "// my own errors\n")?;
    fs::remove_file(dir.path().join("AGENTS.md"))?;

    let output = run_in(dir.path(), &["init"])?;
    assert_success(&output);
    assert_eq!(fs::read_to_string(&error)?, "// my own errors\n");
    assert!(
        dir.path().join("AGENTS.md").is_file(),
        "absent file restored"
    );

    let stdout = stdout(&output);
    assert!(stdout.contains("scaffolded AGENTS.md"), "{stdout}");
    assert!(!stdout.contains("scaffolded src/error.rs"), "{stdout}");
    Ok(())
}

#[test]
fn init_copies_a_template_instead_of_the_skeleton() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_in(dir.path(), &["init", "--template", "actix", "shop"])?;
    assert_success(&output);

    let project = dir.path().join("shop");
    for file in [
        "Cargo.toml",
        "Cargo.lock",
        "README.md",
        "rok-data-sqlc.toml",
        "src/app.rs",
        "src/main.rs",
        "schema/0001_init.sql",
        "migrations/0001_init.up.sql",
        "migrations/0001_init.down.sql",
    ] {
        assert!(project.join(file).is_file(), "{file} is not copied");
    }
    // The template's generated files are left out: `rok gen` writes them.
    assert!(!project.join("src/db").exists(), "generated files copied");
    assert!(!project.join("src/features/generated.rs").exists());
    Ok(())
}

/// A template copy's `migrations/` is the runtime's history: the loader
/// reads the shipped pairs (and not the hidden snapshot beside them).
#[test]
fn a_template_copys_migrations_load_as_history() -> Result<()> {
    let dir = TempDir::new()?;
    assert_success(&run_in(dir.path(), &["init", "--template", "actix"])?);

    let migrations = rok_data::migration_file::load(&dir.path().join("migrations"))?;
    // `init`, then the account flows' `auth` (engine ADR 0020).
    let names: Vec<&str> = migrations.iter().map(|m| m.name).collect();
    assert_eq!(names, ["init", "auth"], "{migrations:?}");
    assert!(
        migrations[1].up.contains("CREATE TABLE \"auth_tokens\""),
        "{}",
        migrations[1].up
    );
    assert!(
        migrations[0].up.contains("CREATE TABLE \"users\""),
        "{}",
        migrations[0].up
    );
    assert!(
        migrations[0].down.contains("DROP TABLE \"users\""),
        "{}",
        migrations[0].down
    );
    Ok(())
}

/// The axum template is the actix one's twin: the same files, generated for
/// the other backend.
#[test]
fn init_copies_the_axum_template() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_in(dir.path(), &["init", "--template", "axum", "shop"])?;
    assert_success(&output);

    let project = dir.path().join("shop");
    for file in [
        "Cargo.toml",
        "Cargo.lock",
        "README.md",
        "rok-data-sqlc.toml",
        "src/app.rs",
        "src/main.rs",
        "schema/0001_init.sql",
        "migrations/0001_init.up.sql",
        "migrations/0001_init.down.sql",
    ] {
        assert!(project.join(file).is_file(), "{file} is not copied");
    }
    // The template generates for axum.
    let config = fs::read_to_string(project.join("rok-data-sqlc.toml"))?;
    assert!(config.contains("extractors = [\"axum\"]"), "{config}");
    assert!(config.contains("framework = \"axum\""), "{config}");
    // The template's generated files are left out: `rok gen` writes them.
    assert!(!project.join("src/db").exists(), "generated files copied");
    assert!(!project.join("src/features/generated.rs").exists());
    Ok(())
}

#[test]
fn init_rejects_an_unknown_template_and_names_the_valid_ones() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_in(dir.path(), &["init", "--template", "rocket"])?;
    assert!(!output.status.success(), "an unknown template must fail");
    let stderr = stderr(&output);
    assert!(
        stderr.contains("rocket") && stderr.contains("actix") && stderr.contains("axum"),
        "{stderr}"
    );
    Ok(())
}

/// The schema directory is the project's only source of truth, so it starts
/// non-empty.
#[test]
fn init_seeds_the_schema_directory() -> Result<()> {
    let dir = TempDir::new()?;
    let schema = dir.path().join("schema.sql");
    fs::write(&schema, "CREATE TABLE users (id UUID PRIMARY KEY);\n")?;

    let output = run_in(
        dir.path(),
        &["init", "--schema", &schema.to_string_lossy(), "app"],
    )?;
    assert_success(&output);

    let seeded = dir.path().join("app/schema/0001_init.sql");
    assert_eq!(
        fs::read_to_string(&seeded)?,
        "CREATE TABLE users (id UUID PRIMARY KEY);\n"
    );
    assert!(
        stdout(&output).contains("seeded "),
        "the seeded file is not reported:\n{}",
        stdout(&output)
    );
    Ok(())
}

/// The scaffolded-file policy holds for the seeded schema too: an existing
/// file is kept, never rewritten.
#[test]
fn init_keeps_a_schema_file_that_is_already_there() -> Result<()> {
    let dir = TempDir::new()?;
    let schema = dir.path().join("schema.sql");
    fs::write(&schema, "CREATE TABLE users (id UUID PRIMARY KEY);\n")?;
    fs::create_dir_all(dir.path().join("schema"))?;
    fs::write(
        dir.path().join("schema/0001_init.sql"),
        "-- mine\nCREATE TABLE notes (id UUID PRIMARY KEY);\n",
    )?;

    let output = run_in(dir.path(), &["init", "--schema", &schema.to_string_lossy()])?;
    assert_success(&output);
    assert_eq!(
        fs::read_to_string(dir.path().join("schema/0001_init.sql"))?,
        "-- mine\nCREATE TABLE notes (id UUID PRIMARY KEY);\n"
    );
    let stdout = stdout(&output);
    assert!(stdout.contains("kept "), "{stdout}");
    assert!(!stdout.contains("seeded "), "{stdout}");
    Ok(())
}

/// The next steps name the new front door, not the legacy engine's command.
#[test]
fn init_names_the_new_front_door() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_in(dir.path(), &["init"])?;
    assert_success(&output);
    let stdout = stdout(&output);
    assert!(stdout.contains("rok gen"), "{stdout}");
    assert!(!stdout.contains("rok sqlc generate"), "{stdout}");

    let steps = next_steps(&output);
    assert!(
        steps.iter().any(|step| step.contains("`rok gen`")),
        "{steps:?}"
    );
    Ok(())
}

#[test]
fn a_template_names_the_new_front_door_too() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_in(dir.path(), &["init", "--template", "actix"])?;
    assert_success(&output);
    let stdout = stdout(&output);
    assert!(stdout.contains("rok gen"), "{stdout}");
    assert!(!stdout.contains("rok sqlc generate"), "{stdout}");
    Ok(())
}

/// A failure reaches the user as `error: …` on stderr, with exit code 1.
#[test]
fn a_failure_is_an_error_on_stderr() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_in(dir.path(), &["init", "--schema", "nope.sql", "app"])?;
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).starts_with("error: "),
        "{}",
        stderr(&output)
    );
    // Nothing was written: the schema is read before the scaffolding.
    assert!(!dir.path().join("app").exists());
    Ok(())
}

#[test]
fn the_help_lists_the_templates() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_in(dir.path(), &["init", "--help"])?;
    assert_success(&output);
    let stdout = stdout(&output);
    for needle in ["--template", "actix", "axum", "--schema"] {
        assert!(stdout.contains(needle), "help lacks {needle}:\n{stdout}");
    }
    Ok(())
}

/// The template the engine ships is the one the root command offers.
#[test]
fn the_offered_templates_are_the_engines() {
    let offered: Vec<&str> = rok_data_sqlc::init::template_names().collect();
    assert_eq!(offered, vec!["actix", "axum"]);
    let engine: PathBuf = common::manifest_dir();
    for template in ["templates/actix-api", "templates/axum-api"] {
        assert!(
            engine.join(template).is_dir(),
            "the offered template is not in the engine"
        );
    }
}
