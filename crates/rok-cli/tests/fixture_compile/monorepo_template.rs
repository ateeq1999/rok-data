//! `templates/monorepo`, made the way a user makes it: `rok init
//! --template monorepo` into a scratch directory, then `rok gen` from
//! the root, which regenerates the api App, re-exports its contract
//! (`apps/api/openapi.json`, through the App's `openapi` example) and
//! emits the Clients from it (workspace ADR 0027). What the template checks
//! in must be exactly what that writes — the api App's generated files,
//! its `openapi.json` and `clients/ts` (set `ROK_DATA_SQLC_UPDATE_EXPECTED=1`
//! to rewrite them) — and the project must pass its own checks: rustfmt,
//! the lint gate, `check-arch`, and `cargo test` (its contract test, its
//! artifact test, and its API tests against
//! `ROK_DATA_SQLC_TEST_DATABASE_URL` when set).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, ensure};

use crate::common::{self, cargo};
use crate::rok_data_sqlc;

/// The monorepo template's directory.
fn template_dir() -> PathBuf {
    common::template_dir("monorepo")
}

/// `rok gen` in `dir`, which must succeed. The contract's export builds
/// the api App: it builds in the gates' shared target directory, with
/// [`cargo`]'s environment.
pub(crate) fn rok_gen_shared(dir: &Path, args: &[&str]) -> Result<String> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rok"));
    command.arg("gen").args(args).current_dir(dir);
    for (key, value) in cargo(dir, &[]).get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        }
    }
    let output = command.output().context("failed to run rok gen")?;
    ensure!(
        output.status.success(),
        "rok gen {args:?} failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A project made from the monorepo template: `init --template monorepo`
/// (which copies the template's `Cargo.lock`, so builds resolve as the
/// template's do), then `rok gen` from the root.
pub(crate) fn materialize() -> Result<tempfile::TempDir> {
    let project = tempfile::TempDir::new()?;
    let dir = project.path();
    rok_data_sqlc(dir, &["init", "--template", "monorepo"])?;
    // Inside the repository the template's `rok-data = "0.5"` resolves to
    // the workspace crate; cargo substitutes it for crates.io while the
    // tests run. The patch shim goes at the workspace root.
    common::rok_data_patch_shim(dir)?;
    rok_gen_shared(dir, &[])?;
    Ok(project)
}

/// rok-data-sqlc's generated files under `root` (a build's `target/`
/// aside), by path relative to `root`, with `\n` line endings.
fn generated_files(root: &Path) -> Result<BTreeMap<PathBuf, String>> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<PathBuf, String>) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            let relative = path.strip_prefix(root)?.to_path_buf();
            if path.is_dir() {
                if relative != Path::new("target") {
                    walk(root, &path, files)?;
                }
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if common::is_generated(&text) {
                files.insert(relative, text.replace("\r\n", "\n"));
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files)?;
    Ok(files)
}

/// The drift between the generated files under `sub` of the project and
/// of the template, each a line; with `ROK_DATA_SQLC_UPDATE_EXPECTED` set,
/// the template's are rewritten instead, and there is none.
fn drift_under(project: &Path, sub: &str) -> Result<Vec<String>> {
    let generated = generated_files(&project.join(sub))?;
    let template = template_dir().join(sub);
    let checked_in = generated_files(&template)?;
    if std::env::var_os("ROK_DATA_SQLC_UPDATE_EXPECTED").is_some() {
        for path in checked_in
            .keys()
            .filter(|path| !generated.contains_key(*path))
        {
            std::fs::remove_file(template.join(path))?;
        }
        for (path, text) in &generated {
            let target = template.join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(target, text)?;
        }
        return Ok(Vec::new());
    }
    Ok(generated
        .keys()
        .chain(checked_in.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| generated.get(*path) != checked_in.get(*path))
        .map(|path| {
            let shown = format!("{sub}/{}", common::shown(path));
            match (generated.contains_key(path), checked_in.contains_key(path)) {
                (true, false) => format!("{shown}: generated, not checked in"),
                (false, true) => format!("{shown}: checked in, no longer generated"),
                _ => format!("{shown}: differs"),
            }
        })
        .collect())
}

/// The template checks in exactly the files `rok gen` generates: the api
/// App's, from its schema and `rok-data-sqlc.toml`, and the TS client's,
/// from the contract. With `ROK_DATA_SQLC_UPDATE_EXPECTED` set, it
/// rewrites them instead.
#[test]
fn monorepo_template_checks_in_what_generation_writes() -> Result<()> {
    let project = materialize()?;
    let generated = generated_files(project.path())?;
    for expected in [
        "apps/api/src/features/users/dto.rs",
        "apps/api/src/db/http/axum.rs",
        "clients/ts/src/calls.ts",
    ] {
        ensure!(
            generated.contains_key(Path::new(expected)),
            "`rok gen` generated no {expected}: {:?}",
            generated.keys()
        );
    }
    let mut drift = drift_under(project.path(), "apps/api")?;
    drift.extend(drift_under(project.path(), "clients/ts")?);
    ensure!(
        drift.is_empty(),
        "templates/monorepo drifted from its generation (regenerate with ROK_DATA_SQLC_UPDATE_EXPECTED=1):\n  {}",
        drift.join("\n  ")
    );
    Ok(())
}

/// The api App's README lists the dependencies and lints of its
/// generated crate; the generated crate compiles and passes its checks.
#[test]
fn monorepo_api_app_compiles_and_passes_checks() -> Result<()> {
    let project = materialize()?;
    // `cargo test -p axum-api` from the root builds and tests the api App.
    // The desktop App, the other Rust member, has its own gate
    // (`desktop_app.rs`): GPUI's build is slow and needs system libraries
    // on Linux, so the Rust matrix does not pay for it here.
    let status = cargo(project.path(), &["test", "-p", "axum-api"]).status()?;
    ensure!(
        status.success(),
        "`cargo test -p axum-api` failed: {status}"
    );
    Ok(())
}

/// The api App's `openapi.json` is the document the App serves, as `rok
/// gen` re-exported it: a contract change re-exports it, nobody edits it.
/// With `ROK_DATA_SQLC_UPDATE_EXPECTED` set, it rewrites the template's
/// copy instead.
#[test]
fn monorepo_template_checks_in_the_exported_contract() -> Result<()> {
    let project = materialize()?;
    let exported = std::fs::read_to_string(project.path().join("apps/api/openapi.json"))?;
    let artifact = template_dir().join("apps/api/openapi.json");
    if std::env::var_os("ROK_DATA_SQLC_UPDATE_EXPECTED").is_some() {
        std::fs::write(&artifact, &exported)?;
        return Ok(());
    }
    let checked_in = std::fs::read_to_string(&artifact)?.replace("\r\n", "\n");
    ensure!(
        checked_in == exported.replace("\r\n", "\n"),
        "templates/monorepo/apps/api/openapi.json is not what `rok gen` exports \
         (regenerate with ROK_DATA_SQLC_UPDATE_EXPECTED=1; never edit it by hand)"
    );
    Ok(())
}
