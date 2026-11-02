//! Where `rok gen` takes its settings: the project files, the flags that
//! beat them, and the legacy commands and sections.

use super::*;

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

/// The flags beat the settings file (workspace ADR 0011).
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
