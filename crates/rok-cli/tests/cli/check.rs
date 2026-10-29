//! `rok sqlc check`: the generated code is up to date, and with `--db`
//! every annotated query agrees with the database (engine ADR 0026).

use super::*;

const QUERIES: &str = "-- name: UserByEmail :maybe_one\n\
     SELECT id, email FROM users WHERE email = $1 LIMIT 1;\n";

/// A project with `users` and one query file, generated.
fn project(queries: &str) -> Result<TempDir> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("schema.sql"), SIMPLE_SCHEMA)?;
    fs::create_dir(dir.path().join("queries"))?;
    fs::write(dir.path().join("queries").join("users.sql"), queries)?;
    fs::write(
        dir.path().join("rok-data-sqlc.toml"),
        "schema = [\"schema.sql\"]\nqueries = [\"queries\"]\noutput = \"out\"\n",
    )?;
    assert_success(&run_binary_in(dir.path(), &["g"]));
    Ok(dir)
}

fn check(dir: &Path, args: &[&str], database_url: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rok"));
    command.current_dir(dir).arg("sqlc").arg("check").args(args);
    match database_url {
        Some(url) => command.env("DATABASE_URL", url),
        None => command.env_remove("DATABASE_URL"),
    };
    command.output().expect("the rok binary runs")
}

#[test]
fn check_passes_when_up_to_date_and_fails_on_drift() -> Result<()> {
    let dir = project(QUERIES)?;
    let output = check(dir.path(), &[], None);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("generated code is up to date"), "{stdout}");

    // A query file edited since the last generation is drift, and `check`
    // writes nothing.
    let path = dir.path().join("queries").join("users.sql");
    fs::write(
        &path,
        format!("{QUERIES}\n-- name: CountUsers :one\nSELECT count(*) AS total FROM users;\n"),
    )?;
    let output = check(dir.path(), &[], None);
    assert!(!output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("changed "), "{stdout}");
    let generated = fs::read_to_string(dir.path().join("out").join("queries").join("users.rs"))?;
    assert!(!generated.contains("count_users"), "check wrote nothing");

    // `c` is its alias.
    assert_success(&run_binary_in(dir.path(), &["g"]));
    assert_success(&run_binary_in(dir.path(), &["c"]));
    Ok(())
}

#[test]
fn a_bare_db_flag_needs_database_url() -> Result<()> {
    let dir = project(QUERIES)?;
    let output = check(dir.path(), &["--db"], None);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("reads DATABASE_URL, which is not set"),
        "{stderr}"
    );
    Ok(())
}

/// A fresh schema on the server `ROK_DATA_SQLC_TEST_DATABASE_URL` names,
/// holding `sql`; answers the URL whose `search_path` is that schema.
fn database(sql: &str) -> Result<Option<String>> {
    let Ok(url) = std::env::var("ROK_DATA_SQLC_TEST_DATABASE_URL") else {
        eprintln!("skipping: ROK_DATA_SQLC_TEST_DATABASE_URL is not set");
        return Ok(None);
    };
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let schema = format!("rok_check_{}_{nanos}", std::process::id());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let separator = if url.contains('?') { '&' } else { '?' };
    let scoped = format!("{url}{separator}options=-c%20search_path%3D{schema}");
    runtime.block_on(async {
        let db = rok_data::Db::connect(&url).await?;
        rok_data::raw(format!("CREATE SCHEMA {schema}"))
            .execute(&db)
            .await?;
        let db = rok_data::Db::connect(&scoped).await?;
        for statement in sql.split(';').filter(|s| !s.trim().is_empty()) {
            rok_data::raw(statement).execute(&db).await?;
        }
        anyhow::Ok(())
    })?;
    Ok(Some(scoped))
}

#[test]
fn check_db_compares_every_query_with_the_database() -> Result<()> {
    let Some(url) = database(SIMPLE_SCHEMA)? else {
        return Ok(());
    };
    let dir = project(QUERIES)?;
    let output = check(dir.path(), &["--db"], Some(&url));
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("1 query agree with the database"),
        "{stdout}"
    );

    // A parameter typed other than the database infers it is a mismatch at
    // its query's line; the queries that agree are not listed.
    let queries = format!(
        "{QUERIES}\n-- name: ById :many\n-- param: $1 id i64\nSELECT email FROM users WHERE id = $1;\n\n\
         -- name: Total :one\nSELECT count(*) AS total FROM users;\n"
    );
    let dir = project(&queries)?;
    let output = check(dir.path(), &["-d", &url], None);
    assert!(!output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(
            "users.sql:4: ById: parameter `$1` (`id`) is `UUID` in the database; the generated code uses `i64`"
        ),
        "{stdout}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("1 mismatch(es) in 3 queries"), "{stderr}");
    Ok(())
}
