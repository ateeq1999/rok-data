//! Errors show their causes: a failing command prints its error with the
//! whole chain, so "failed to parse SQL schema" names the file, the line and
//! the parser's message — with `--json` too, whose stdout stays the report.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

fn rok(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rok-data"))
        .current_dir(dir)
        .args(args)
        .env_remove("DATABASE_URL")
        .output()
        .expect("spawn rok")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A plain project whose second schema line is not SQL.
fn broken_schema() -> TempDir {
    let dir = TempDir::new().expect("temp dir");
    fs::write(
        dir.path().join("rok-data-sqlc.toml"),
        "schema = \"schema\"\nmigrations = \"migrations\"\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("schema")).unwrap();
    fs::write(
        dir.path().join("schema/0001_init.sql"),
        "CREATE TABLE users (id UUID PRIMARY KEY);\nCREATE TABLE posts (id UUID PRIMARY KEY,, title TEXT);\n",
    )
    .unwrap();
    dir
}

/// The `error:` line, which must carry the file, the line and the parser's
/// own message after the context it was wrapped in.
fn assert_parse_error_with_its_chain(output: &Output) {
    assert!(!output.status.success(), "{}", stderr(output));
    let line = stderr(output)
        .lines()
        .find(|line| line.starts_with("error: "))
        .map(str::to_owned)
        .unwrap_or_else(|| panic!("no `error:` line:\n{}", stderr(output)));
    assert!(
        line.starts_with(
            "error: failed to parse SQL schema: schema/0001_init.sql:2: invalid PostgreSQL SQL syntax: "
        ),
        "{line}"
    );
    // The parser's message, after the place.
    assert!(line.contains("Expected"), "{line}");
}

#[test]
fn a_schema_syntax_error_names_the_file_the_line_and_the_parsers_message() {
    let dir = broken_schema();
    assert_parse_error_with_its_chain(&rok(dir.path(), &["gen", "--dry-run"]));
    assert_parse_error_with_its_chain(&rok(dir.path(), &["gen", "check"]));
}

#[test]
fn with_json_the_error_carries_its_chain_too() {
    let dir = broken_schema();
    let output = rok(dir.path(), &["gen", "--dry-run", "--json"]);
    assert_parse_error_with_its_chain(&output);
    let _: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout stays one JSON document");
}
