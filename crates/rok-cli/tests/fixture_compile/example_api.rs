//! `examples/api`: the vertical slice — the axum template's hand-written
//! half, a truth of two files, and beside it the generated data layer,
//! the API features and the staged migration history.
//!
//! The example is a crate of its own (the root `Cargo.toml`'s `exclude`
//! lists it, and it has its own `[workspace]` table), so this suite is
//! what proves it: the generated code matches the schema (`rok gen check`),
//! the staged migration pairs are the ones `rok gen --migration` writes,
//! and the crate passes its own checks (rustfmt, the lint gate, `cargo
//! test`).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, ensure};

use crate::common::{self, cargo};

/// The repository's root: the directory holding the workspace's
/// `Cargo.toml` and the `crates/` beside it.
fn repo_root() -> PathBuf {
    let mut dir = std::env::current_dir().expect("run from a directory");
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("crates/rok-data").is_dir() {
            return dir;
        }
        dir = dir
            .parent()
            .expect("run inside the rok-data workspace")
            .to_owned();
    }
}

/// The example crate: the repository's `examples/api`.
fn example_dir() -> PathBuf {
    repo_root().join("examples/api")
}

/// `rok <args>` in `dir`, which must succeed.
fn rok(dir: &Path, args: &[&str]) -> Result<std::process::Output> {
    let output = Command::new(env!("CARGO_BIN_EXE_rok"))
        .args(args)
        .current_dir(dir)
        .output()
        .context("failed to run rok")?;
    ensure!(
        output.status.success(),
        "rok {args:?} failed in {}:\n{}{}",
        dir.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}

/// Copies `from` into `to`, without the `target` directory.
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        if entry.file_name() == "target" {
            continue;
        }
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// A file's text, with its line endings normalized to `\n`.
fn read(path: &Path) -> Result<String> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(text.replace("\r\n", "\n"))
}

/// The generated code matches the schema: `rok gen check` writes nothing
/// and finds no drift.
#[test]
fn the_example_is_in_sync_with_its_schema() -> Result<()> {
    let dir = example_dir();
    let output = rok(&dir, &["gen", "check"])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("up to date"),
        "rok gen check does not report the example as up to date:\n{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// The staged migration pairs are the ones `rok gen --migration` writes:
/// staged again from the same truth, they are the same files.
#[test]
fn the_example_stages_the_migration_pairs_it_checks_in() -> Result<()> {
    let checked_in = example_dir();
    let project = tempfile::TempDir::new()?;
    let dir = project.path().join("api");
    copy_tree(&checked_in, &dir)?;

    // The pairs and the snapshot go from the history dir: the staging
    // starts from nothing.
    let truth_schema = dir.join("schema");
    let history = dir.join("migrations");
    for entry in std::fs::read_dir(&history)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".up.sql") || name.ends_with(".down.sql") || name.starts_with('.') {
            std::fs::remove_file(entry.path())?;
        }
    }
    // The second truth file waits: the first migration carries the first
    // file's tables alone. Its configuration entry goes with it, since the
    // table is not in the schema yet.
    let loans = truth_schema.join("loans.sql");
    let truth = std::fs::read_to_string(&loans)?;
    std::fs::remove_file(&loans)?;
    let config = dir.join("rok.toml");
    let settings = std::fs::read_to_string(&config)?;
    let reduced = settings.replace("loans = [\"destroy\"]\n", "");
    ensure!(
        reduced != settings,
        "the example's configuration no longer names the loans' skipped endpoint"
    );
    std::fs::write(&config, &reduced)?;

    rok(&dir, &["gen", "--migration", "init"])?;
    std::fs::write(&loans, &truth)?;
    std::fs::write(&config, &settings)?;
    rok(&dir, &["gen", "--migration", "add_loans"])?;

    for name in [
        "0001_init.up.sql",
        "0001_init.down.sql",
        "0002_add_loans.up.sql",
        "0002_add_loans.down.sql",
    ] {
        let staged = read(&history.join(name))?;
        let expected = read(&checked_in.join("migrations").join(name))?;
        ensure!(
            staged == expected,
            "the staged {name} differs from the one the example checks in"
        );
    }
    Ok(())
}

/// The example is a crate of its own, and it passes its own checks:
/// rustfmt, the lint gate, and its tests (the API tests against
/// PostgreSQL when a database is configured; they skip otherwise).
#[test]
fn the_example_passes_its_checks_and_tests() -> Result<()> {
    let dir = example_dir();
    if let Some(reason) = common::generated_code_skip_reason(&dir) {
        eprintln!("{reason}");
        return Ok(());
    }
    let fmt = Command::new("cargo")
        .args(["fmt", "--check"])
        .current_dir(&dir)
        .output()
        .context("failed to run cargo fmt")?;
    ensure!(
        fmt.status.success(),
        "the example is not rustfmt-clean:\n{}",
        String::from_utf8_lossy(&fmt.stdout)
    );
    let database = std::env::var("ROK_DATA_SQLC_TEST_DATABASE_URL").ok();
    for (args, what) in [
        (
            vec!["clippy", "--quiet", "--all-targets", "--", "-D", "warnings"],
            "clippy",
        ),
        (vec!["test", "--quiet"], "test"),
    ] {
        let mut command = cargo(&dir, &args);
        match &database {
            Some(url) => command.env("DATABASE_URL", url),
            None => command.env_remove("DATABASE_URL"),
        };
        let output = command.output().context("failed to run cargo")?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        ensure!(
            output.status.success() && !stderr.contains("warning:"),
            "the example fails `cargo {what}`:\n{}{stderr}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    Ok(())
}
