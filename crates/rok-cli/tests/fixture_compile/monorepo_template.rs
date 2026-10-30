//! `templates/monorepo`, made the way a user makes it: `rok init
//! --template monorepo` into a scratch directory, then `rok gen` from
//! the root. The generated files the template checks in under
//! `apps/api/` must be exactly what `g` writes (set
//! `ROK_DATA_SQLC_UPDATE_EXPECTED=1` to rewrite them), and the project
//! must pass its own checks: rustfmt, the lint gate, `check-arch`, and
//! `cargo test` (its contract test, and its API tests against
//! `ROK_DATA_SQLC_TEST_DATABASE_URL` when set).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};

use crate::common::{self, cargo};
use crate::rok_data_sqlc;

/// The monorepo template's directory.
fn template_dir() -> PathBuf {
    common::template_dir("monorepo")
}

/// A project made from the monorepo template: `init --template monorepo`
/// (which copies the template's `Cargo.lock`, so builds resolve as the
/// template's do), then `rok gen` from the root.
fn materialize() -> Result<tempfile::TempDir> {
    let project = tempfile::TempDir::new()?;
    let dir = project.path();
    rok_data_sqlc(dir, &["init", "--template", "monorepo"])?;
    // Inside the repository the template's `rok-data = "0.5"` resolves to
    // the workspace crate; cargo substitutes it for crates.io while the
    // tests run. The patch shim goes at the workspace root.
    common::rok_data_patch_shim(dir)?;
    // `rok gen` from the root regenerates the Apps the `[apps]` section names.
    common::rok_gen(dir, &[])?;
    Ok(project)
}

/// rok-data-sqlc's generated files under `root` (a build's `target/`
/// aside), by relative path, with `\n` line endings. We only care about
/// the Rust files under `apps/api/` (the monorepo's api App).
fn generated_files(root: &Path) -> Result<BTreeMap<PathBuf, String>> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<PathBuf, String>) -> Result<()> {
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

/// The template checks in exactly the files `rok gen` generates from the
/// api App's schema and `rok-data-sqlc.toml`. With
/// `ROK_DATA_SQLC_UPDATE_EXPECTED` set, it rewrites them instead.
#[test]
fn monorepo_template_checks_in_what_generation_writes() -> Result<()> {
    let project = materialize()?;
    let generated = generated_files(project.path())?;
    // The api App's generated files live under `apps/api/`.
    ensure!(
        generated
            .keys()
            .any(|p| p.starts_with("apps/api/src/features/users/dto.rs")),
        "`rok gen` generated no users feature under apps/api/: {:?}",
        generated.keys()
    );
    ensure!(
        generated
            .keys()
            .any(|p| p.starts_with("apps/api/src/db/http/axum.rs")),
        "`rok gen` generated no axum extractor under apps/api/: {:?}",
        generated.keys()
    );
    // Filter generated files to the api App, stripping the `apps/api/` prefix.
    let generated_api: BTreeMap<PathBuf, String> = generated
        .into_iter()
        .filter(|(path, _)| path.starts_with("apps/api/"))
        .map(|(path, text)| {
            let stripped = path.strip_prefix("apps/api/").unwrap();
            (stripped.to_path_buf(), text)
        })
        .collect();
    // Compare against what the template checks in under apps/api/.
    let checked_in = generated_files(&template_dir().join("apps/api"))?;
    if std::env::var_os("ROK_DATA_SQLC_UPDATE_EXPECTED").is_some() {
        for path in checked_in
            .keys()
            .filter(|path| !generated_api.contains_key(*path))
        {
            std::fs::remove_file(template_dir().join("apps/api").join(path))?;
        }
        for (path, text) in &generated_api {
            let target = template_dir().join("apps/api").join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(target, text)?;
        }
        return Ok(());
    }
    let drift: Vec<String> = generated_api
        .keys()
        .chain(checked_in.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|path| generated_api.get(*path) != checked_in.get(*path))
        .map(|path| {
            match (
                generated_api.contains_key(path),
                checked_in.contains_key(path),
            ) {
                (true, false) => format!("{}: generated, not checked in", path.display()),
                (false, true) => format!("{}: checked in, no longer generated", path.display()),
                _ => format!("{}: differs", path.display()),
            }
        })
        .collect();
    ensure!(
        drift.is_empty(),
        "templates/monorepo/apps/api drifted from its generation (regenerate with ROK_DATA_SQLC_UPDATE_EXPECTED=1):\n  {}",
        drift.join("\n  ")
    );
    Ok(())
}

/// The api App's README lists the dependencies and lints of its
/// generated crate; the generated crate compiles and passes its checks.
#[test]
fn monorepo_api_app_compiles_and_passes_checks() -> Result<()> {
    let project = materialize()?;
    // `cargo test --workspace` from the root builds and tests the api App.
    let status = cargo(project.path(), &["test", "--workspace"]).status()?;
    ensure!(
        status.success(),
        "`cargo test --workspace` failed: {status}"
    );
    Ok(())
}
