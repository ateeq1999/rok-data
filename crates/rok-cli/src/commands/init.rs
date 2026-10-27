//! `rok init [NAME]`: scaffold a project — the application skeleton, or a
//! ready-made template — and seed its schema directory.
//!
//! The scaffolding itself lives in the engine (`rok_data_sqlc::init`), the
//! same library `rok sqlc init` uses: every file is written once and never
//! rewritten. This module owns the root command's own parts: the project
//! directory, the seeded schema, and the report.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rok_data_sqlc::ApiFramework;
use rok_data_sqlc::init::{Scaffolded, init as init_skeleton, init_template};

use crate::output::display_path;

/// The truth directory of a project: its only source of truth
/// (workspace ADR 0010).
const SCHEMA_DIR: &str = "schema";

/// The file a seeded schema is written as.
const SCHEMA_FILE: &str = "0001_init.sql";

/// `rok init [NAME] [--template <NAME> | --api-framework <FW>] [--schema <PATH>]`.
pub(crate) fn run(
    name: Option<PathBuf>,
    template: Option<String>,
    api_framework: Option<&str>,
    schema: Option<PathBuf>,
) -> Result<ExitCode, String> {
    let dir = name.unwrap_or_else(|| PathBuf::from("."));
    let is_template = template.is_some();
    // The schema is read before anything is written: a path that does not
    // exist must not leave a half-scaffolded project behind.
    let content = match schema.as_deref() {
        Some(schema) => Some(read_schema(schema)?),
        None => None,
    };
    let scaffolded = match template {
        Some(template) => init_template(&dir, &template).map_err(|err| format!("{err:#}"))?,
        None => {
            let framework = match api_framework {
                Some("axum") => ApiFramework::Axum,
                _ => ApiFramework::ActixWeb,
            };
            init_skeleton(&dir, framework).map_err(|err| format!("{err:#}"))?
        }
    };
    report(&scaffolded);
    let seeded = match content {
        Some(content) => seed_schema(&dir, &content)?,
        None => false,
    };
    next_steps(&scaffolded, is_template, seeded);
    Ok(ExitCode::SUCCESS)
}

/// Prints what the scaffolder wrote.
fn report(scaffolded: &Scaffolded) {
    if scaffolded.replaced_default_main {
        println!("replaced cargo's default src/main.rs");
    }
    for path in &scaffolded.written {
        println!("scaffolded {}", display_path(path));
    }
}

/// The content of the schema file the project's schema directory is seeded
/// with.
fn read_schema(schema: &Path) -> Result<String, String> {
    fs::read_to_string(schema)
        .map_err(|err| format!("failed to read `{}`: {err}", schema.display()))
}

/// Seeds the project's truth directory with `content`.
///
/// The schema directory is the project's only source of truth, so it starts
/// non-empty: the file is written as `schema/0001_init.sql`. Like every
/// scaffolded file it is written once and never rewritten — an existing one
/// is kept, and the run says so. Answers whether it wrote the file.
fn seed_schema(dir: &Path, content: &str) -> Result<bool, String> {
    let target = dir.join(SCHEMA_DIR).join(SCHEMA_FILE);
    if target.exists() {
        println!("kept {} (already there)", display_path(&target));
        return Ok(false);
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create directory `{}`: {err}", parent.display()))?;
    }
    fs::write(&target, content)
        .map_err(|err| format!("failed to write `{}`: {err}", target.display()))?;
    println!("seeded {}", display_path(&target));
    Ok(true)
}

/// Prints the numbered next steps.
///
/// The front door is `rok gen`: the truth in the schema directory is the
/// truth (workspace ADR 0010), and `rok gen` turns it into the data
/// layer and the API features; `rok gen --migration` stages the history
/// pairs into `migrations/`, the default history dir.
fn next_steps(scaffolded: &Scaffolded, template: bool, seeded: bool) {
    let mut steps: Vec<&str> = Vec::new();
    if template {
        if !scaffolded.wrote_manifest {
            steps.push(
                "add the template's dependencies and lints (its README.md lists them) to Cargo.toml",
            );
        }
        steps.push("run `rok gen` to generate src/db/, src/shared/ and src/features/ from schema/");
        steps.push("set DATABASE_URL and JWT_SECRET (see .env.example) and apply migrations/");
        steps.push("`cargo test`, then `cargo run`");
    } else {
        steps.push("add the dependencies listed at the end of rok-data-sqlc.toml to Cargo.toml");
        if seeded {
            steps.push(
                "grow the truth in schema/: edit its .sql files, then stage the change with `rok gen --migration <name>`",
            );
        } else {
            steps.push(
                "describe the tables in schema/ (0001_init.sql), then stage them with `rok gen --migration init`",
            );
        }
        steps.push("run `rok gen` to generate src/db/ and src/features/");
        steps.push("set DATABASE_URL and JWT_SECRET, then `cargo run`");
    }
    println!("\nNext steps:");
    for (number, step) in steps.iter().enumerate() {
        println!("  {}. {step}", number + 1);
    }
}
