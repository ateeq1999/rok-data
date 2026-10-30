//! The `apps/web` App of the monorepo template: the TanStack Start web app
//! vendored from `ateeq1999/tanstack-starter-axum`. The gate installs deps
//! with `bun install --frozen-lockfile`, runs `typecheck` and `test`, and
//! skips with a printed note when the bun toolchain is absent.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use crate::common::{self};
use crate::rok_data_sqlc;

/// The monorepo template's web App directory.
#[allow(dead_code)]
fn web_app_dir() -> PathBuf {
    common::template_dir("monorepo").join("apps/web")
}

/// A scaffolded monorepo project with the web App: `init --template monorepo`
/// then `rok gen` (which for the monorepo means the api App's generation).
fn materialize() -> Result<tempfile::TempDir> {
    let project = tempfile::TempDir::new()?;
    let dir = project.path();
    rok_data_sqlc(dir, &["init", "--template", "monorepo"])?;
    common::rok_data_patch_shim(dir)?;
    // `rok gen` from the root regenerates the Apps the `[apps]` section names.
    common::rok_gen(dir, &[])?;
    Ok(project)
}

/// Whether the `bun` command is available.
fn bun_available() -> bool {
    std::process::Command::new("bun")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// `bun <args>` in `dir`. Returns `Ok(())` on success, `Err` with the
/// combined stderr+stdout on failure.
fn bun(dir: &Path, args: &[&str]) -> Result<()> {
    let output = std::process::Command::new("bun")
        .args(args)
        .current_dir(dir)
        .output()
        .context("failed to run bun")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        output.status.success(),
        "bun {args:?} failed:\n{stdout}{stderr}"
    );
    Ok(())
}

/// The gate: scaffold the monorepo, install the web app's deps, typecheck,
/// and test. Skips with a printed note when bun is absent.
#[test]
fn monorepo_web_app_installs_typechecks_and_tests() -> Result<()> {
    if !bun_available() {
        eprintln!("note: skipping web app gate; `bun` not available (install bun to run it)");
        return Ok(());
    }
    let project = materialize()?;
    let web_dir = project.path().join("apps/web");
    // Install with the locked dependencies.
    bun(&web_dir, &["install", "--frozen-lockfile"])?;
    // Typecheck.
    bun(&web_dir, &["run", "typecheck"])?;
    // Test.
    bun(&web_dir, &["test"])?;
    Ok(())
}
