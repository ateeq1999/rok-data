//! The `rok generate` grammar and its option merging: the write mode keeps
//! the flat `rok generate [OPTIONS]` shape of the original `rok-db-gen`,
//! the CI mode nests as `rok generate check`, flags given at both levels
//! resolve with the nested leaf winning, and the original rule that
//! `check` rejects `--migration`/`--rename` stays.

use std::fs;
use std::process::Command;

use tempfile::TempDir;

fn rok() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rok"))
}

const DECISIONS: &str = "CREATE TABLE decisions (\n    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),\n    text TEXT NOT NULL DEFAULT '' ::text\n);\n";

/// Generate a small crate, then check it in every spelling there is.
#[test]
fn write_and_check_share_one_grammar() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let project = dir.path();
    fs::create_dir_all(project.join("sql"))?;
    fs::write(project.join("sql/decisions.sql"), DECISIONS)?;

    // The flat write shape.
    let status = rok()
        .current_dir(project)
        .args(["generate", "--input", "sql", "--output", "gen"])
        .status()?;
    assert!(status.success(), "flat generate run failed");
    assert!(project.join("gen/src/lib.rs").is_file());

    // The check spellings: nested leaf with its own flags, and group flags
    // before the nested leaf. All must find the crate fresh.
    for args in [
        vec!["generate", "check", "--input", "sql", "--output", "gen"],
        vec!["generate", "--input", "sql", "--output", "gen", "check"],
    ] {
        let output = rok().current_dir(project).args(&args).output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "`{args:?}` failed: {stdout}{stderr}"
        );
        assert!(
            stdout.contains("is up to date"),
            "unexpected check output for {args:?}: {stdout}{stderr}"
        );
    }

    // Flags at both levels with different values: the nested leaf must win.
    // (The group's `--input db` does not exist; if it won, this would fail.)
    let output = rok()
        .current_dir(project)
        .args([
            "generate", "--input", "db", "check", "--input", "sql", "--output", "gen",
        ])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("is up to date"),
        "the leaf must win over the group: {}{}",
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

/// The original rule: `check` rejected `--migration` and `--rename`; with
/// those flags now living on the group, they must still be refused under
/// `check` instead of silently ignored.
#[test]
fn check_rejects_migration_and_rename_flags() {
    for flag in ["--migration", "--rename"] {
        let output = rok()
            .arg("generate")
            .arg(flag)
            .arg("anything")
            .arg("check")
            .output()
            .expect("failed to run rok");
        assert!(
            !output.status.success(),
            "`generate {flag} … check` must fail"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(flag),
            "the error must name the flag: {stderr}"
        );
    }
}

/// Without a nested action and with no options at all, `rok generate` runs
/// with its documented defaults; against an empty directory that means an
/// input error, not a usage error.
#[test]
fn bare_generate_uses_the_documented_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let output = rok().current_dir(dir.path()).arg("generate").output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("error:"),
        "a failing generate run explains itself on stderr: {stderr}"
    );
    Ok(())
}

/// The help text advertises both shapes.
#[test]
fn help_shows_the_nested_check() {
    let output = rok().args(["generate", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for needle in ["[OPTIONS] [COMMAND]", "check"] {
        assert!(stdout.contains(needle), "help lists {needle}: {stdout}");
    }
}
