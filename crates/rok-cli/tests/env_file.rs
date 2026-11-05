//! `rok` loads the App's `.env` for its database commands: `rok migrate`,
//! `rok db` and `rok schema` read `<app>/.env` of the App the one rule finds
//! (the `.env` beside a plain project's settings file). A variable already
//! in the environment wins, the command's `using` line says `loaded
//! apps/api/.env`, and `--no-env-file` reads nothing.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

/// `rok <args>` in `dir`, with no `DATABASE_URL` of the caller's.
fn rok(dir: &Path, args: &[&str]) -> Output {
    rok_with(dir, args, None)
}

/// `rok <args>` in `dir`, with `DATABASE_URL` exported as `url` (or unset).
fn rok_with(dir: &Path, args: &[&str], url: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rok-data"));
    command
        .current_dir(dir)
        .args(args)
        .env_remove("DATABASE_URL");
    if let Some(url) = url {
        command.env("DATABASE_URL", url);
    }
    command.output().expect("spawn rok")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A Monorepo whose `[apps]` names `apps/api`, whose `.env` sets
/// `DATABASE_URL` to `url`.
fn monorepo(url: &str) -> TempDir {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("rok.toml"), "[apps]\napi = \"apps/api\"\n").unwrap();
    let app = dir.path().join("apps/api");
    fs::create_dir_all(app.join("migrations")).unwrap();
    fs::write(
        app.join("rok-data-sqlc.toml"),
        "schema = \"schema\"\nmigrations = \"migrations\"\n",
    )
    .unwrap();
    fs::write(
        app.join(".env"),
        format!("# the App's settings\nDATABASE_URL={url}\nJWT_SECRET=dev\n"),
    )
    .unwrap();
    dir
}

/// A database URL nothing answers on, named so the echo shows which it was.
fn nowhere(name: &str) -> String {
    format!("postgres://127.0.0.1:1/{name}")
}

const USING_AT_ROOT: &str = "using App api: apps/api/rok-data-sqlc.toml (migrations apps/api/migrations), loaded apps/api/.env";

fn said(output: &Output, line: &str) -> bool {
    stderr(output).lines().any(|said| said == line)
}

#[test]
fn migrate_at_a_monorepo_root_reads_the_apps_env_file() {
    let dir = monorepo(&nowhere("from_file"));
    let output = rok(dir.path(), &["migrate", "status"]);
    assert!(said(&output, USING_AT_ROOT), "{}", stderr(&output));
    // It had a database to ask for, from the file: it tried to connect.
    assert!(
        !stderr(&output).contains("needs a database"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("can't connect to the database"),
        "{}",
        stderr(&output)
    );

    // Inside the App, the same file.
    let output = rok(&dir.path().join("apps/api"), &["schema", "diff"]);
    assert!(
        said(
            &output,
            "using App api: rok-data-sqlc.toml (migrations migrations), loaded .env"
        ),
        "{}",
        stderr(&output)
    );
}

#[test]
fn an_exported_variable_wins_over_the_env_file() {
    let dir = monorepo(&nowhere("from_file"));
    let output = rok_with(
        dir.path(),
        &["db", "create", "scratch"],
        Some(&nowhere("from_shell")),
    );
    assert!(said(&output, "loaded apps/api/.env"), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("/from_shell"),
        "{}",
        stderr(&output)
    );
    assert!(
        !stderr(&output).contains("/from_file"),
        "{}",
        stderr(&output)
    );

    // Without the export, the file's.
    let output = rok(dir.path(), &["db", "create", "scratch"]);
    assert!(
        stderr(&output).contains("/from_file"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn no_env_file_reads_nothing() {
    let dir = monorepo(&nowhere("from_file"));
    for args in [
        &["migrate", "up", "--no-env-file"][..],
        &["migrate", "--no-env-file", "status"],
        &["schema", "diff", "--no-env-file"],
        &["schema", "dump", "--no-env-file"],
        &["db", "check", "--no-env-file"],
    ] {
        let output = rok(dir.path(), args);
        assert!(
            stderr(&output).contains("needs a database"),
            "{args:?}: {}",
            stderr(&output)
        );
        assert!(
            !stderr(&output).contains("loaded "),
            "{args:?}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn db_and_schema_dump_without_a_project_still_run() {
    let dir = TempDir::new().unwrap();
    // A `.env` with no project around it is not the App's: not read.
    fs::write(
        dir.path().join(".env"),
        format!("DATABASE_URL={}\n", nowhere("stray")),
    )
    .unwrap();
    for args in [&["db", "check"][..], &["schema", "dump"]] {
        let output = rok(dir.path(), args);
        assert!(
            stderr(&output).contains("needs a database"),
            "{args:?}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn a_plain_project_reads_the_env_file_beside_its_settings() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("rok-data-sqlc.toml"),
        "migrations = \"migrations\"\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".env"),
        format!("DATABASE_URL={}\n", nowhere("plain")),
    )
    .unwrap();
    let inside = dir.path().join("src");
    fs::create_dir_all(&inside).unwrap();
    let output = rok(&inside, &["db", "create", "scratch"]);
    // The settings file above the current directory is shown as the
    // project rule shows it, so only the file's name is pinned here.
    assert!(
        stderr(&output)
            .lines()
            .any(|line| line.starts_with("loaded ") && line.ends_with("/.env")),
        "{}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("/plain"), "{}", stderr(&output));
}

/// `rok migrate up` at a Monorepo root applies the api App's migrations to
/// the database its `.env` names, with no `DATABASE_URL` exported.
#[test]
fn migrate_up_at_a_monorepo_root_uses_the_apps_env_database() {
    let Some(base) = std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty()) else {
        eprintln!(
            "skipping migrate_up_at_a_monorepo_root_uses_the_apps_env_database: DATABASE_URL not set"
        );
        return;
    };
    let name = format!("rok_t_env_file_{}", std::process::id());
    let mut admin = url::Url::parse(&base).expect("DATABASE_URL is a URL");
    admin.set_path("/postgres");
    let mut url = admin.clone();
    url.set_path(&format!("/{name}"));

    let dir = monorepo(url.as_str());
    let migrations = dir.path().join("apps/api/migrations");
    fs::write(
        migrations.join("0001_init.up.sql"),
        "CREATE TABLE env_users (id INT PRIMARY KEY);\n",
    )
    .unwrap();
    fs::write(
        migrations.join("0001_init.down.sql"),
        "DROP TABLE env_users;\n",
    )
    .unwrap();

    let created = rok(
        dir.path(),
        &["db", "create", &name, "--database-url", admin.as_str()],
    );
    let output = rok(dir.path(), &["migrate", "up", "--json"]);
    let dropped = rok(
        dir.path(),
        &[
            "db",
            "drop",
            &name,
            "--database-url",
            admin.as_str(),
            "--yes",
            "--force",
        ],
    );

    assert!(created.status.success(), "{}", stderr(&created));
    assert!(
        output.status.success(),
        "{}{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(said(&output, USING_AT_ROOT), "{}", stderr(&output));
    let applied: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON stdout");
    assert_eq!(
        applied,
        serde_json::json!({ "applied": [{ "version": 1, "name": "init" }] })
    );
    assert!(dropped.status.success(), "{}", stderr(&dropped));
}
