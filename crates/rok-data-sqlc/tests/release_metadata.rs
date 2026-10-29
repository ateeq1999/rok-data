//! Release metadata: what `cargo package` ships, and that the declared
//! minimum supported Rust version is the one CI tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, ensure};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn packaged_files() -> Result<Vec<String>> {
    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["package", "--list", "--allow-dirty", "--offline"])
        .current_dir(root())
        .output()
        .context("failed to run cargo package --list")?;
    ensure!(
        output.status.success(),
        "cargo package --list failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?
        .lines()
        .map(|line| line.replace('\\', "/"))
        .collect())
}

#[test]
fn package_ships_licenses_readme_and_changelog_but_no_fixtures() -> Result<()> {
    let files = packaged_files()?;
    for required in [
        "Cargo.toml",
        "README.md",
        "CHANGELOG.md",
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "src/lib.rs",
        // The engine ships as a library; the user-facing `rok sqlc …`
        // commands live in the workspace's rok-cli crate. (Consolidation
        // removed this crate's own binary target, so no src/main.rs.)
        // `init` embeds its scaffold with `include_str!`; `init --template`'s
        // templates are the `rok-templates` crate's.
        "src/init/scaffold/AGENTS.md",
        "src/init/scaffold/rok-data-sqlc.toml",
    ] {
        ensure!(
            files.iter().any(|file| file == required),
            "{required} missing from the package: {files:?}"
        );
    }
    for excluded in [".scratch/", "modules/", "tests/", "docs/", ".github/"] {
        ensure!(
            !files.iter().any(|file| file.starts_with(excluded)),
            "{excluded} must not be packaged: {files:?}"
        );
    }
    Ok(())
}

/// `package.rust-version`: in the workspace's manifests it is inherited
/// (`rust-version.workspace = true`), so fall back to the workspace's
/// `[workspace.package]` table.
fn rust_version(manifest_dir: &Path) -> Result<String> {
    let manifest: toml::Table = fs::read_to_string(manifest_dir.join("Cargo.toml"))?.parse()?;
    manifest["package"]
        .get("rust-version")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .or_else(|| {
            let root = manifest_dir.join("../../Cargo.toml");
            let workspace: toml::Table = fs::read_to_string(root).ok()?.parse().ok()?;
            workspace
                .get("workspace")
                .and_then(|ws| ws.get("package"))
                .and_then(|package| package.get("rust-version"))
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
        .with_context(|| {
            format!(
                "{} must declare package.rust-version",
                manifest_dir.display()
            )
        })
}

/// The workspace's minimum supported Rust version and the higher one
/// generated code needs (SQLx 0.9's). After the consolidation they are the
/// same (1.94): the CI matrix lists it once, plus stable. If the tool's own
/// floor ever drops below the generated code's again, the matrix must list
/// both.
#[test]
fn ci_tests_both_declared_minimum_rust_versions() -> Result<()> {
    let msrv = rust_version(&root())?;
    let generated_floor = rust_version(&root().join("tests/fixture-app"))?;
    ensure!(
        rust_version(&root().join("tests/modules-app"))? == generated_floor,
        "tests/modules-app must declare the same rust-version as tests/fixture-app ({generated_floor})"
    );
    ensure!(
        rust_version(&root().join("tests/api-app"))? == generated_floor,
        "tests/api-app must declare the same rust-version as tests/fixture-app ({generated_floor})"
    );
    ensure!(
        generated_floor == "1.94",
        "generated code targets SQLx 0.9, whose MSRV is 1.94; the fixture crates declare {generated_floor}"
    );
    let workspace_root = root().join("../..");
    let workflow = fs::read_to_string(workspace_root.join(".github/workflows/ci.yml"))
        .with_context(|| {
            format!(
                "the workspace CI at {} moved or was removed",
                workspace_root.display()
            )
        })?;
    let matrix = if msrv == generated_floor {
        format!("rust: [\"{msrv}\", stable]")
    } else {
        format!("rust: [\"{msrv}\", \"{generated_floor}\", stable]")
    };
    ensure!(
        workflow.contains(&matrix),
        "the workspace CI matrix must test the tool's rust-version {msrv} \
         and generated code's floor {generated_floor}, plus stable; expected `{matrix}`"
    );
    Ok(())
}
