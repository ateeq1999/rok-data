//! `rok sqlc query add`: one query appended and generated in one step, kept
//! only when it generates.

use super::*;

/// A project with `users`, a `queries/` directory holding `users.sql`, and
/// a `rok-data-sqlc.toml` naming both.
fn project() -> Result<TempDir> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("schema.sql"), SIMPLE_SCHEMA)?;
    fs::create_dir(dir.path().join("queries"))?;
    fs::write(
        dir.path().join("queries").join("users.sql"),
        "-- name: CountUsers :one\nSELECT count(*) AS total FROM users;\n",
    )?;
    fs::write(
        dir.path().join("rok-data-sqlc.toml"),
        "schema = [\"schema.sql\"]\nqueries = [\"queries\"]\noutput = \"out\"\n",
    )?;
    Ok(dir)
}

#[test]
fn adds_a_query_and_generates_its_function() -> Result<()> {
    let dir = project()?;
    let output = run_binary_in(
        dir.path(),
        &[
            "query",
            "add",
            "UserByEmail",
            "SELECT * FROM users WHERE email = $1 LIMIT 1",
            "--file",
            "users",
        ],
    );
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("added `UserByEmail` to"), "{stdout}");

    // The query joins the file, its mode inferred from `LIMIT 1`.
    let file = fs::read_to_string(dir.path().join("queries").join("users.sql"))?;
    assert!(
        file.ends_with(
            "\n\n-- name: UserByEmail :maybe_one\nSELECT * FROM users WHERE email = $1 LIMIT 1;\n"
        ),
        "{file}"
    );
    let generated = fs::read_to_string(dir.path().join("out").join("queries").join("users.rs"))?;
    assert!(
        generated.contains("pub async fn user_by_email<'e, E>(executor: E, email: &str)"),
        "{generated}"
    );
    assert!(generated.contains("-> Result<Option<User>, rok_data::Error>"));

    // A mode flag overrides the inference.
    let output = run_binary_in(
        dir.path(),
        &[
            "q",
            "add",
            "AllEmails",
            "SELECT email FROM users",
            "--file",
            "users",
            "--many",
        ],
    );
    assert_success(&output);
    let file = fs::read_to_string(dir.path().join("queries").join("users.sql"))?;
    assert!(file.contains("-- name: AllEmails :many\n"), "{file}");
    Ok(())
}

#[test]
fn a_query_that_does_not_generate_is_not_kept() -> Result<()> {
    let dir = project()?;
    let path = dir.path().join("queries").join("users.sql");
    let before = fs::read_to_string(&path)?;
    let output = run_binary_in(
        dir.path(),
        &[
            "query",
            "add",
            "Broken",
            "SELECT nickname FROM users",
            "--file",
            "users",
        ],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("`Broken` was not added to"), "{stderr}");
    assert!(stderr.contains("nickname"), "{stderr}");
    assert_eq!(fs::read_to_string(&path)?, before, "the file is restored");

    // A new file that doesn't generate is removed again.
    let output = run_binary_in(
        dir.path(),
        &[
            "query",
            "add",
            "Broken",
            "SELECT nickname FROM users",
            "--file",
            "fresh",
        ],
    );
    assert!(!output.status.success());
    assert!(!dir.path().join("queries").join("fresh.sql").exists());
    Ok(())
}

#[test]
fn names_must_be_new_and_pascal_case() -> Result<()> {
    let dir = project()?;
    let output = run_binary_in(
        dir.path(),
        &["query", "add", "CountUsers", "SELECT 1", "--file", "users"],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already has a query named `CountUsers`"),
        "{stderr}"
    );

    let output = run_binary_in(
        dir.path(),
        &["query", "add", "count_users", "SELECT 1", "--file", "users"],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("is not PascalCase"), "{stderr}");

    // Two mode flags are a usage error.
    let output = run_binary_in(
        dir.path(),
        &[
            "query", "add", "X", "SELECT 1", "--file", "users", "--one", "--many",
        ],
    );
    assert!(!output.status.success());
    Ok(())
}

#[test]
fn the_sql_is_read_from_a_file_named_with_an_at() -> Result<()> {
    let dir = project()?;
    // Quotes that a shell would fight over.
    fs::write(
        dir.path().join("q.sql"),
        "SELECT * FROM users\nWHERE email = $1 AND email <> 'it''s'\nLIMIT 1;\n",
    )?;
    let output = run_binary_in(
        dir.path(),
        &["query", "add", "UserByEmail", "@q.sql", "--file", "users"],
    );
    assert_success(&output);
    let file = fs::read_to_string(dir.path().join("queries").join("users.sql"))?;
    assert!(
        file.ends_with(
            "-- name: UserByEmail :maybe_one\nSELECT * FROM users\nWHERE email = $1 AND email <> 'it''s'\nLIMIT 1;\n"
        ),
        "{file}"
    );
    let generated = fs::read_to_string(dir.path().join("out").join("queries").join("users.rs"))?;
    assert!(
        generated.contains("pub async fn user_by_email"),
        "{generated}"
    );
    Ok(())
}

#[test]
fn the_sql_is_read_from_stdin_with_a_dash() -> Result<()> {
    use std::io::Write;
    use std::process::Stdio;

    let dir = project()?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_rok-data"))
        .arg("sqlc")
        .current_dir(dir.path())
        .args(["query", "add", "AllEmails", "-", "--file", "users"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(b"SELECT email FROM users WHERE email <> 'x'\n")?;
    let output = child.wait_with_output()?;
    assert_success(&output);
    let file = fs::read_to_string(dir.path().join("queries").join("users.sql"))?;
    assert!(
        file.ends_with("-- name: AllEmails :many\nSELECT email FROM users WHERE email <> 'x';\n"),
        "{file}"
    );
    Ok(())
}

#[test]
fn a_missing_sql_file_is_named() -> Result<()> {
    let dir = project()?;
    let path = dir.path().join("queries").join("users.sql");
    let before = fs::read_to_string(&path)?;
    let output = run_binary_in(
        dir.path(),
        &[
            "query",
            "add",
            "UserByEmail",
            "@nope.sql",
            "--file",
            "users",
        ],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("nope.sql"), "{stderr}");
    assert_eq!(fs::read_to_string(&path)?, before);
    Ok(())
}
