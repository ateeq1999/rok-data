//! `rok gen` and the API features: which tables it serves, and what it
//! says when that changes.

use super::*;

/// A `rok.toml` serving the schema's tables under `src/features`, with
/// `api_extra` appended to its `[api]` section.
fn api_project(schema: &str, api_extra: &str) -> Result<TempDir> {
    let dir = project(schema)?;
    fs::write(
        dir.path().join("rok.toml"),
        format!(
            "[generate]\nschema = \"migrations/\"\nderive = [\"serde\", \"validate\"]\n\
             extractors = [\"actix_web\"]\n\n[api]\noutput = \"src/features\"\n{api_extra}"
        ),
    )?;
    Ok(dir)
}

/// The run serving a table for the first time says so once, on stderr and
/// in `--json`'s `newly_served`; the next run doesn't.
#[test]
fn a_newly_served_table_is_announced_once() -> Result<()> {
    let dir = api_project(SCHEMA, "")?;
    let output = run_in(dir.path(), &["gen"])?;
    assert_success(&output);
    let first = stderr(&output);
    let line = "warning: now serving `/api/v1/users` (list, create, get_one, update, remove); \
                add `users` to `[api] exclude` to keep it private";
    assert_eq!(first.matches(line).count(), 1, "{first}");

    fs::write(
        dir.path().join("migrations/0001_init.sql"),
        SCHEMA_WITH_POSTS,
    )?;
    let output = run_in(dir.path(), &["gen", "--json"])?;
    assert_success(&output);
    let stdout = stdout(&output);
    let report: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    assert_eq!(
        report["newly_served"],
        serde_json::json!([{
            "table": "posts",
            "route": "/api/v1/posts",
            "endpoints": ["list", "create", "get_one", "update", "remove"],
            "message": "now serving `/api/v1/posts` (list, create, get_one, update, remove); \
                        add `posts` to `[api] exclude` to keep it private",
        }]),
        "{stdout}"
    );

    let output = run_in(dir.path(), &["gen"])?;
    assert_success(&output);
    let again = stderr(&output);
    assert!(!again.contains("now serving"), "{again}");
    Ok(())
}

/// `[api] include` serves only the tables it names; with `exclude` beside
/// it the run fails naming both.
#[test]
fn include_is_an_allowlist_and_excludes_exclude() -> Result<()> {
    let dir = api_project(SCHEMA_WITH_POSTS, "include = [\"posts\"]\n")?;
    let output = run_in(dir.path(), &["gen"])?;
    assert_success(&output);
    assert!(dir.path().join("src/features/posts/mod.rs").is_file());
    assert!(!dir.path().join("src/features/users").exists());

    let dir = api_project(
        SCHEMA_WITH_POSTS,
        "include = [\"posts\"]\nexclude = [\"users\"]\n",
    )?;
    let output = run_in(dir.path(), &["gen"])?;
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    assert!(
        stderr.contains("`api.include`") && stderr.contains("`api.exclude`"),
        "{stderr}"
    );
    Ok(())
}
