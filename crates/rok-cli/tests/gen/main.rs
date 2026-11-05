//! `rok gen` / `rok gen check`: the unified front door. It generates the
//! data layer and the API features from the project's schema dir, and
//! stages a migration pair when it is asked to.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use anyhow::Result;
use tempfile::TempDir;

/// A schema with one table.
const SCHEMA: &str =
    "CREATE TABLE users (\n    id UUID PRIMARY KEY,\n    email TEXT NOT NULL\n);\n";

/// A schema with a second table.
const SCHEMA_WITH_POSTS: &str = "CREATE TABLE users (\n    id UUID PRIMARY KEY,\n    email TEXT NOT NULL\n);\n\nCREATE TABLE posts (\n    id UUID PRIMARY KEY,\n    title TEXT NOT NULL\n);\n";

fn rok() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rok-data"))
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

/// A project whose schema dir holds `schema`.
fn project(schema: &str) -> Result<TempDir> {
    let dir = TempDir::new()?;
    fs::create_dir_all(dir.path().join("migrations"))?;
    fs::write(dir.path().join("migrations/0001_init.sql"), schema)?;
    Ok(dir)
}

/// The migrations of a project's schema dir, by name.
fn migrations(dir: &Path) -> Result<Vec<String>> {
    let mut names: Vec<String> = fs::read_dir(dir.join("migrations"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".sql") && !name.starts_with('.'))
        .collect();
    names.sort();
    Ok(names)
}

mod check;
mod generate;
mod json;
mod next_steps;
mod settings;
mod staging;
mod steps;
