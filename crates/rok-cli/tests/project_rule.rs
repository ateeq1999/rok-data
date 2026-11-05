//! One rule for finding the project: every command that reads a project
//! walks up from the current directory to the nearest `rok.toml`; inside an
//! App that `[apps]` names it acts on that App, at the root on the App(s)
//! `[apps]` names, and an explicit `--dir` still wins. Each says which
//! settings file and directories it read, in one line on stderr.

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

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        stdout(output),
        stderr(output)
    );
}

/// A Monorepo whose `[apps]` names `apps/api`, the App keeping its truth in
/// `schema/` and its history in `migrations/`, as the template scaffolds it.
fn monorepo() -> TempDir {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("rok.toml"), "[apps]\napi = \"apps/api\"\n").unwrap();
    let app = dir.path().join("apps/api");
    fs::create_dir_all(app.join("schema")).unwrap();
    fs::create_dir_all(app.join("migrations")).unwrap();
    fs::write(
        app.join("rok-data-sqlc.toml"),
        "schema = \"schema\"\nmigrations = \"migrations\"\n\
         derive = [\"serde\", \"validate\"]\nextractors = [\"axum\"]\n\n\
         [api]\noutput = \"src/features\"\nframework = \"axum\"\n",
    )
    .unwrap();
    fs::write(
        app.join("schema/0001_init.sql"),
        "CREATE TABLE users (id UUID PRIMARY KEY, email TEXT NOT NULL);\n",
    )
    .unwrap();
    dir
}

/// The names in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn migrate_create_at_a_monorepo_root_writes_into_the_apps_migrations() {
    let dir = monorepo();
    let output = rok(dir.path(), &["migrate", "create", "add_posts"]);
    assert_success(&output);
    assert_eq!(
        names(&dir.path().join("apps/api/migrations")),
        ["0001_add_posts.down.sql", "0001_add_posts.up.sql"]
    );
    assert!(!dir.path().join("migrations").exists());
    assert_said(
        &output,
        "using App api: apps/api/rok-data-sqlc.toml (migrations apps/api/migrations)",
    );
}

/// The line the command printed on stderr, and nothing of it on stdout.
fn assert_said(output: &Output, line: &str) {
    assert!(
        stderr(output).lines().any(|said| said == line),
        "expected `{line}` on stderr:\n{}",
        stderr(output)
    );
    assert!(!stdout(output).contains("using "), "{}", stdout(output));
}

#[test]
fn migrate_create_inside_the_app_writes_into_the_same_migrations() {
    let dir = monorepo();
    let app = dir.path().join("apps/api");
    let output = rok(&app, &["migrate", "create", "add_posts"]);
    assert_success(&output);
    assert_said(
        &output,
        "using App api: rok-data-sqlc.toml (migrations migrations)",
    );

    // Deeper inside the App, the same App's directory.
    let inside = app.join("src");
    fs::create_dir_all(&inside).unwrap();
    let output = rok(&inside, &["migrate", "create", "add_tags"]);
    assert_success(&output);
    assert!(!inside.join("migrations").exists());
    assert_eq!(
        names(&app.join("migrations")),
        [
            "0001_add_posts.down.sql",
            "0001_add_posts.up.sql",
            "0002_add_tags.down.sql",
            "0002_add_tags.up.sql"
        ]
    );
    assert!(
        stderr(&output).contains("using App api: "),
        "{}",
        stderr(&output)
    );
}

#[test]
fn an_explicit_dir_wins_over_the_project() {
    let dir = monorepo();
    let output = rok(
        dir.path(),
        &["migrate", "create", "add_posts", "--dir", "elsewhere"],
    );
    assert_success(&output);
    assert_eq!(
        names(&dir.path().join("apps/api/migrations")),
        Vec::<String>::new()
    );
    assert_eq!(
        names(&dir.path().join("elsewhere")),
        ["0001_add_posts.down.sql", "0001_add_posts.up.sql"]
    );
    assert_said(&output, "using --dir (migrations elsewhere)");
}

/// A second App beside `api`, without settings of its own.
fn add_app(dir: &Path, name: &str) {
    fs::write(
        dir.join("rok.toml"),
        format!("[apps]\napi = \"apps/api\"\n{name} = \"apps/{name}\"\n"),
    )
    .unwrap();
    fs::create_dir_all(dir.join("apps").join(name)).unwrap();
}

#[test]
fn several_apps_at_the_root_need_app() {
    let dir = monorepo();
    add_app(dir.path(), "admin");
    let output = rok(dir.path(), &["migrate", "create", "add_posts"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("this Monorepo names 2 Apps (admin, api): pass --app <name>"),
        "{}",
        stderr(&output)
    );

    let output = rok(
        dir.path(),
        &["migrate", "create", "add_posts", "--app", "admin"],
    );
    assert_success(&output);
    assert_eq!(
        names(&dir.path().join("apps/admin/migrations")),
        ["0001_add_posts.down.sql", "0001_add_posts.up.sql"]
    );
    assert_said(
        &output,
        "using App admin: no settings file (migrations apps/admin/migrations)",
    );
}

#[test]
fn an_unknown_app_names_the_apps_there_are() {
    let dir = monorepo();
    let output = rok(dir.path(), &["migrate", "create", "x", "--app", "web"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("--app `web`: rok.toml names no such App (its `[apps]`: api)"),
        "{}",
        stderr(&output)
    );
}

/// The commands that need a database say what they read before they ask
/// for one, at the root and inside the App alike.
#[test]
fn migrate_and_schema_diff_read_the_app_at_the_root_and_inside_it() {
    let dir = monorepo();
    let app = dir.path().join("apps/api");
    for args in [
        &["migrate", "status"][..],
        &["migrate", "up", "--json"],
        &["migrate", "down"],
        &["schema", "diff"],
    ] {
        let output = rok(dir.path(), args);
        assert_said(
            &output,
            "using App api: apps/api/rok-data-sqlc.toml (migrations apps/api/migrations)",
        );
        let output = rok(&app, args);
        assert_said(
            &output,
            "using App api: rok-data-sqlc.toml (migrations migrations)",
        );
        assert!(
            stderr(&output).contains("needs a database"),
            "{}",
            stderr(&output)
        );
    }
}

#[test]
fn gen_and_gen_check_read_the_app_at_the_root_and_inside_it() {
    let dir = monorepo();
    let app = dir.path().join("apps/api");
    for args in [
        &["gen", "--dry-run", "--json"][..],
        &["gen", "check", "--json"],
    ] {
        let output = rok(dir.path(), args);
        assert_said(
            &output,
            "using App api: apps/api/rok-data-sqlc.toml (schema apps/api/schema, migrations apps/api/migrations)",
        );
        let output = rok(&app, args);
        assert_said(
            &output,
            "using App api: rok-data-sqlc.toml (schema schema, migrations migrations)",
        );
    }
}

#[test]
fn check_arch_finds_the_app_at_the_root_and_inside_it() {
    let dir = monorepo();
    let app = dir.path().join("apps/api");
    fs::create_dir_all(app.join("src/features")).unwrap();
    let output = rok(dir.path(), &["sqlc", "check-arch"]);
    assert_success(&output);
    assert_said(
        &output,
        "using App api: apps/api/rok-data-sqlc.toml (features apps/api/src/features)",
    );
    let output = rok(&app, &["sqlc", "check-arch"]);
    assert_success(&output);
    assert_said(
        &output,
        "using App api: rok-data-sqlc.toml (features src/features)",
    );
}

#[test]
fn check_arch_finds_a_plain_rok_toml_project() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("rok.toml"),
        "[api]\noutput = \"src/features\"\nframework = \"axum\"\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("src/features")).unwrap();
    let output = rok(dir.path(), &["sqlc", "check-arch"]);
    assert_success(&output);
    assert_said(&output, "using rok.toml (features src/features)");
}

/// `DATABASE_URL` pointed at database `name`.
fn url_for(base: &str, name: &str) -> String {
    let mut url = url::Url::parse(base).expect("DATABASE_URL is a URL");
    url.set_path(&format!("/{name}"));
    url.to_string()
}

/// `rok migrate up` at a Monorepo root applies the api App's migrations,
/// in a database of its own that it drops again.
#[test]
fn migrate_up_at_a_monorepo_root_migrates_the_api_app() {
    let Some(base) = std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty()) else {
        eprintln!(
            "skipping migrate_up_at_a_monorepo_root_migrates_the_api_app: DATABASE_URL not set"
        );
        return;
    };
    let dir = monorepo();
    fs::write(
        dir.path().join("apps/api/migrations/0001_init.up.sql"),
        "CREATE TABLE rule_users (id INT PRIMARY KEY);\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("apps/api/migrations/0001_init.down.sql"),
        "DROP TABLE rule_users;\n",
    )
    .unwrap();
    let name = format!("rok_t_project_rule_{}", std::process::id());
    let admin = url_for(&base, "postgres");
    assert_success(&rok(
        dir.path(),
        &["db", "create", &name, "--database-url", &admin],
    ));
    let url = url_for(&base, &name);

    let output = rok(
        dir.path(),
        &["migrate", "up", "--json", "--database-url", &url],
    );
    let status = rok(
        &dir.path().join("apps/api"),
        &["migrate", "status", "--json", "--database-url", &url],
    );
    let dropped = rok(
        dir.path(),
        &["db", "drop", &name, "--database-url", &admin, "--yes"],
    );

    assert_success(&output);
    assert_said(
        &output,
        "using App api: apps/api/rok-data-sqlc.toml (migrations apps/api/migrations)",
    );
    let applied: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON stdout");
    assert_eq!(
        applied,
        serde_json::json!({ "applied": [{ "version": 1, "name": "init" }] })
    );
    assert_success(&status);
    assert!(
        stdout(&status).contains("\"applied\""),
        "{}",
        stdout(&status)
    );
    assert_success(&dropped);
}
