//! The `apps/web` App of the monorepo template, made the way a user makes
//! it: `rok init --template monorepo` into a scratch directory, then, in
//! the scaffolded `apps/web`, the web App's own checks with its own
//! tooling — `bun install --frozen-lockfile` (the template's `bun.lock`
//! must match its `package.json`), `bun run typecheck` and `bun run test`
//! (the App's vitest suite; `bun test`, bun's own runner, is not it).
//! Skips with a printed note when bun is absent.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, ensure};

use crate::common;
use crate::rok_data_sqlc;

/// The `bun` to run: the one on `PATH`, else the one bun's installer puts
/// under `~/.bun/bin`; `None` when neither runs.
fn bun() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| {
            PathBuf::from(home).join(format!(".bun/bin/bun{}", std::env::consts::EXE_SUFFIX))
        });
    std::iter::once(PathBuf::from("bun"))
        .chain(home)
        .find(|bun| {
            Command::new(bun)
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success())
        })
}

/// The major version of the `node` on `PATH`, when there is one.
fn node_major() -> Option<u32> {
    let output = Command::new("node").arg("--version").output().ok()?;
    let version = String::from_utf8(output.stdout).ok()?;
    version
        .trim()
        .strip_prefix('v')?
        .split('.')
        .next()?
        .parse()
        .ok()
}

/// `bun <args>` in `dir`, which must succeed.
fn run(bun: &Path, dir: &Path, args: &[&str]) -> Result<()> {
    let mut command = Command::new(bun);
    command.args(args).current_dir(dir);
    // Node 25 turned Web Storage on by default: its global `localStorage`
    // (unusable without `--localstorage-file`) shadows jsdom's in vitest,
    // and the App's session tests read `window.localStorage`. Turned off,
    // jsdom's storage is the one the tests see, as on the Node the App was
    // written for.
    if node_major().is_some_and(|major| major >= 25) {
        command.env("NODE_OPTIONS", "--no-experimental-webstorage");
    }
    let output = command.output().context("failed to run bun")?;
    ensure!(
        output.status.success(),
        "bun {args:?} in {} failed:\n{}{}",
        common::shown(dir),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// The scaffold writes the web App's binary files byte for byte, and its
/// `.env.example`.
#[test]
fn monorepo_scaffold_writes_the_web_app_whole() -> Result<()> {
    let project = tempfile::TempDir::new()?;
    rok_data_sqlc(project.path(), &["init", "--template", "monorepo"])?;
    let template = common::template_dir("monorepo").join("apps/web");
    let web = project.path().join("apps/web");
    for file in ["public/favicon.ico", "public/logo.png"] {
        ensure!(
            std::fs::read(web.join(file))? == std::fs::read(template.join(file))?,
            "the scaffolded apps/web/{file} differs from the template's"
        );
    }
    for file in [
        ".env.example",
        "package.json",
        "bun.lock",
        "vitest.config.ts",
    ] {
        ensure!(
            web.join(file).is_file(),
            "the scaffold has no apps/web/{file}"
        );
    }
    Ok(())
}

/// The gate: scaffold the monorepo, then install the web App's locked
/// dependencies, typecheck it and run its tests.
#[test]
fn monorepo_web_app_installs_typechecks_and_tests() -> Result<()> {
    let Some(bun) = bun() else {
        eprintln!(
            "note: skipping the web App gate: `bun` is not installed (https://bun.sh installs it)"
        );
        return Ok(());
    };
    let project = tempfile::TempDir::new()?;
    rok_data_sqlc(project.path(), &["init", "--template", "monorepo"])?;
    let web = project.path().join("apps/web");
    run(&bun, &web, &["install", "--frozen-lockfile"])?;
    run(&bun, &web, &["run", "typecheck"])?;
    run(&bun, &web, &["run", "test"])?;
    Ok(())
}
