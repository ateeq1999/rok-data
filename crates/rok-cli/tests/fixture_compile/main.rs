//! Generates the fixture schemas into the fixture crates and checks the
//! output the way a consuming crate would see it: `tests/fixture-app` (the
//! data layer), `tests/api-app`, `tests/axum-app` and `tests/grpc-app` (the
//! API features, one per framework) and a project made with
//! `rok sqlc init` and `rok sqlc generate`.
//!
//! Set `ROK_DATA_SQLC_TEST_DATABASE_URL` to a throwaway PostgreSQL database to
//! also execute the generated code (see `tests/fixture-app/tests/postgres/`).

#[path = "../common/mod.rs"]
mod common;

mod api_app;
mod axum_app;
mod example_api;
mod fixture_app;
mod grpc_app;
mod init;

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, ensure};

/// Runs `rok-data-sqlc <args>` in `dir`, which must succeed, and returns its
/// stderr.
fn rok_data_sqlc(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_rok-data"))
        .arg("sqlc")
        .args(args)
        .current_dir(dir)
        .output()
        .context("failed to run rok-data-sqlc")?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    ensure!(
        output.status.success(),
        "rok-data-sqlc {args:?} failed:\n{}{stderr}",
        String::from_utf8_lossy(&output.stdout),
    );
    Ok(stderr)
}

/// Whether `stdout` is only the line a passing `rok sqlc check-arch` prints
/// (`checked 3 features: no violations`), with no violation and no warning.
fn check_arch_passed(stdout: &[u8]) -> bool {
    let stdout = String::from_utf8_lossy(stdout);
    let mut lines = stdout.lines();
    lines
        .next()
        .is_some_and(|line| line.starts_with("checked ") && line.ends_with(": no violations"))
        && lines.next().is_none()
}
