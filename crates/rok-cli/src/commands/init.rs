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

/// `rok init [NAME] [--template <NAME> [--org <ORG>] | --api-framework <FW>] [--schema <PATH>]`.
pub(crate) fn run(
    name: Option<PathBuf>,
    template: Option<String>,
    org: Option<&str>,
    api_framework: Option<&str>,
    schema: Option<PathBuf>,
) -> Result<ExitCode, String> {
    let dir = name.unwrap_or_else(|| PathBuf::from("."));
    // The schema is read before anything is written: a path that does not
    // exist must not leave a half-scaffolded project behind.
    let content = match schema.as_deref() {
        Some(schema) => Some(read_schema(schema)?),
        None => None,
    };
    let scaffolded = match template.as_deref() {
        Some(template) => init_template(&dir, template, org).map_err(|err| format!("{err:#}"))?,
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
    next_steps(&dir, &scaffolded, template.as_deref(), seeded);
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

/// Whether `dir` is the current directory.
fn is_current_dir(dir: &Path) -> bool {
    let full = |path: &Path| path.canonicalize().ok();
    full(dir).is_some_and(|dir| Some(dir) == full(Path::new(".")))
}

/// Prints the numbered next steps.
///
/// The front door is `rok gen`: the truth in the schema directory is the
/// truth (workspace ADR 0010), and `rok gen` turns it into the data
/// layer and the API features; `rok gen --migration` stages the history
/// pairs into `migrations/`, the default history dir. A template's steps
/// are the engine's, so `rok sqlc init` prints the same ones (a Monorepo
/// template's name its Apps).
fn next_steps(dir: &Path, scaffolded: &Scaffolded, template: Option<&str>, seeded: bool) {
    let mut steps = match template {
        Some(template) => scaffolded.template_next_steps(template, "rok gen"),
        None => {
            let schema = if seeded {
                "grow the truth in schema/: edit its .sql files, then stage the change with `rok gen --migration <name>`"
            } else {
                "describe the tables in schema/ (0001_init.sql), then stage them with `rok gen --migration init`"
            };
            [
                "add the dependencies listed at the end of rok-data-sqlc.toml to Cargo.toml",
                schema,
                "run `rok gen` to generate src/db/ and src/features/",
                "set DATABASE_URL and JWT_SECRET, then `cargo run`",
            ]
            .map(str::to_owned)
            .to_vec()
        }
    };
    // The commands run as printed from where `rok init` ran: a project
    // made elsewhere is `cd`ed into first.
    if !is_current_dir(dir) {
        let shown = display_path(dir);
        let shown = if shown.contains(char::is_whitespace) {
            format!("\"{shown}\"")
        } else {
            shown
        };
        steps.insert(0, format!("`cd {shown}`"));
    }
    println!("\nNext steps:");
    for (number, step) in steps.iter().enumerate() {
        println!("  {}. {step}", number + 1);
    }
}
