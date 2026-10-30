//! `rok db create|drop` and the connection resolution the server commands
//! share (`--database-url` > `--server` > `DATABASE_URL`), driven through
//! the `rok` binary (ADR 0028).
//!
//! Every database a test creates or drops is its own (`rok_t_…`); the one
//! `DATABASE_URL` names is never dropped. The "project database" of the
//! guard tests is a throwaway set as the child's `DATABASE_URL`.

use std::io::Write;
use std::process::{Output, Stdio};

use crate::db_support::{
    Sandbox, TestDb, assert_error, assert_success, database_exists, database_url_or_skip, json,
    stderr, stdout, unique_name, url_for,
};

/// A URL nothing listens on: a command that connects through it fails.
const NOWHERE: &str = "postgres://nobody:nothing@127.0.0.1:1/none";

/// Runs `rok <args>` in `sandbox` with `input` on stdin.
fn run_with_input(sandbox: &Sandbox, args: &[&str], input: &str) -> Output {
    let mut child = sandbox
        .rok(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rok");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait for rok")
}

/// The variable `rok` reads a server's password from.
fn password_var(server: &str) -> String {
    format!(
        "ROK_SERVER_{}_PASSWORD",
        server.to_uppercase().replace('-', "_")
    )
}

/// A sandbox with server `name` registered on `base`'s host, connecting to
/// the `postgres` maintenance database; the password goes in through the
/// variable on each command (see [`on_server`]).
fn sandbox_with_server(base: &url::Url, name: &str) -> Sandbox {
    let sandbox = Sandbox::new();
    sandbox.add_server(name, &url_for(base, "postgres"));
    sandbox
}

/// `rok <args>` against server `name`, its password from the variable.
fn on_server(sandbox: &Sandbox, base: &url::Url, name: &str, args: &[&str]) -> Output {
    sandbox
        .rok(args)
        .env(password_var(name), base.password().unwrap_or_default())
        .output()
        .expect("spawn rok")
}

/// `SELECT current_database()` through `rok db query`, as the command
/// resolves its connection.
fn current_database(output: &Output) -> String {
    assert_success(output);
    json(output)["results"][0]["rows"][0][0]
        .as_str()
        .expect("a database name")
        .to_owned()
}

#[test]
fn db_create_and_drop_require_a_name() {
    let sandbox = Sandbox::new();
    for command in ["create", "drop"] {
        let output = sandbox.run(&["db", command]);
        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
        assert!(stderr(&output).contains("<NAME>"), "{}", stderr(&output));
    }
}

#[test]
fn db_commands_without_any_connection_say_what_to_set() {
    let sandbox = Sandbox::new();
    for args in [
        &["db", "create", "x"][..],
        &["db", "drop", "x", "--yes"],
        &["db", "query", "SELECT 1"],
    ] {
        assert_error(
            &sandbox.run(args),
            "set DATABASE_URL or pass --database-url",
        );
    }
}

#[test]
fn db_commands_with_an_unknown_server_fail() {
    let sandbox = Sandbox::new();
    for args in [
        &["db", "create", "x", "--server", "nope"][..],
        &["db", "drop", "x", "--server", "nope", "--yes"],
        &["db", "query", "SELECT 1", "--server", "nope"],
    ] {
        assert_error(&sandbox.run(args), "server 'nope' not found");
    }
}

#[test]
fn db_command_on_a_server_without_a_password_names_the_variable() {
    let sandbox = Sandbox::new();
    sandbox.add_server("box", NOWHERE);
    let output = sandbox.run(&["db", "query", "SELECT 1", "--server", "box"]);
    assert_error(&output, "ROK_SERVER_BOX_PASSWORD not set");
}

#[test]
fn db_errors_never_print_the_password() {
    let sandbox = Sandbox::new();
    sandbox.add_server("box", NOWHERE);
    let output = sandbox
        .rok(&["db", "create", "x", "--server", "box"])
        .env("ROK_SERVER_BOX_PASSWORD", "s3cret-pw")
        .output()
        .expect("spawn rok");
    assert_error(&output, "can't connect to the database");
    assert!(
        !stderr(&output).contains("s3cret-pw"),
        "{}",
        stderr(&output)
    );
    assert!(
        !stdout(&output).contains("s3cret-pw"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn db_drop_without_yes_and_a_wrong_name_drops_nothing() {
    // Refused at the prompt, before connecting.
    let output = run_with_input(
        &Sandbox::new(),
        &["db", "drop", "kept", "--database-url", NOWHERE],
        "not-kept\n",
    );
    assert_error(&output, "confirmation failed: database name does not match");
    assert!(
        !stderr(&output).contains("can't connect"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn db_create_on_server() {
    let Some(base) = database_url_or_skip("db_create_on_server") else {
        return;
    };
    let sandbox = sandbox_with_server(&base, "qa");
    let name = unique_name("create_server");

    let output = on_server(
        &sandbox,
        &base,
        "qa",
        &["db", "create", &name, "--server", "qa"],
    );
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!("Database '{name}' created on server 'qa'.\n")
    );
    assert!(database_exists(&base, &name));

    let dropped = on_server(
        &sandbox,
        &base,
        "qa",
        &["db", "drop", &name, "--server", "qa", "--yes"],
    );
    assert_success(&dropped);
}

#[test]
fn db_drop_on_server() {
    let Some(base) = database_url_or_skip("db_drop_on_server") else {
        return;
    };
    let sandbox = sandbox_with_server(&base, "qa");
    let name = unique_name("drop_server");
    assert_success(&on_server(
        &sandbox,
        &base,
        "qa",
        &["db", "create", &name, "--server", "qa"],
    ));

    let output = on_server(
        &sandbox,
        &base,
        "qa",
        &["db", "drop", &name, "--server", "qa", "--yes"],
    );
    assert_success(&output);
    assert_eq!(stdout(&output), format!("Database '{name}' dropped.\n"));
    assert!(!database_exists(&base, &name));
}

#[test]
fn db_drop_typed_confirmation() {
    let Some(base) = database_url_or_skip("db_drop_typed_confirmation") else {
        return;
    };
    let admin = url_for(&base, "postgres");
    let name = unique_name("typed");
    let sandbox = Sandbox::new();
    assert_success(&sandbox.run(&["db", "create", &name, "--database-url", &admin]));

    // A different name keeps the database.
    let refused = run_with_input(
        &sandbox,
        &["db", "drop", &name, "--database-url", &admin],
        "something_else\n",
    );
    assert_error(&refused, "confirmation failed");
    assert!(database_exists(&base, &name));

    // The exact name drops it; the prompt stays off stdout.
    let output = run_with_input(
        &sandbox,
        &["db", "drop", &name, "--database-url", &admin, "--json"],
        &format!("{name}\n"),
    );
    assert_success(&output);
    assert!(stderr(&output).contains("Type the database name to confirm: "));
    assert_eq!(
        json(&output),
        serde_json::json!({ "database": name, "dropped": true })
    );
    assert!(!database_exists(&base, &name));
}

#[test]
fn db_drop_yes_flag() {
    let Some(base) = database_url_or_skip("db_drop_yes_flag") else {
        return;
    };
    let admin = url_for(&base, "postgres");
    let name = unique_name("yes");
    let sandbox = Sandbox::new();
    assert_success(&sandbox.run(&["db", "create", &name, "--database-url", &admin]));

    // `-y` needs no input at all (stdin is closed).
    let output = sandbox
        .rok(&["db", "drop", &name, "--database-url", &admin, "-y"])
        .stdin(Stdio::null())
        .output()
        .expect("spawn rok");
    assert_success(&output);
    assert!(!stdout(&output).contains("confirm"), "{}", stdout(&output));
    assert!(!database_exists(&base, &name));
}

#[test]
fn db_drop_project_db_refused() {
    let Some(base) = database_url_or_skip("db_drop_project_db_refused") else {
        return;
    };
    let project = TestDb::create(&base, "project");
    let sandbox = sandbox_with_server(&base, "qa");

    // Refused through --database-url and through --server alike, even
    // with --yes and a differently-cased name.
    let upper = project.name.to_uppercase();
    for args in [
        vec![
            "db",
            "drop",
            &project.name,
            "--database-url",
            NOWHERE,
            "--yes",
        ],
        vec!["db", "drop", &upper, "--server", "qa", "--yes"],
    ] {
        let output = sandbox
            .rok(&args)
            .env("DATABASE_URL", &project.url)
            .env(password_var("qa"), base.password().unwrap_or_default())
            .output()
            .expect("spawn rok");
        assert_error(
            &output,
            "refusing to drop the project's own database (DATABASE_URL). Use --force to override.",
        );
    }
    assert!(database_exists(&base, &project.name));
}

#[test]
fn db_drop_project_db_force() {
    let Some(base) = database_url_or_skip("db_drop_project_db_force") else {
        return;
    };
    // Created here rather than as a `TestDb`: the test itself drops it.
    let admin = url_for(&base, "postgres");
    let name = unique_name("project_force");
    let sandbox = Sandbox::new();
    assert_success(&sandbox.run(&["db", "create", &name, "--database-url", &admin]));

    let output = sandbox
        .rok(&[
            "db",
            "drop",
            &name,
            "--database-url",
            &admin,
            "--yes",
            "--force",
        ])
        .env("DATABASE_URL", url_for(&base, &name))
        .output()
        .expect("spawn rok");
    assert_success(&output);
    assert!(!database_exists(&base, &name));
}

#[test]
fn db_create_drop_json() {
    let Some(base) = database_url_or_skip("db_create_drop_json") else {
        return;
    };
    let sandbox = sandbox_with_server(&base, "qa");
    let name = unique_name("json");

    let created = on_server(
        &sandbox,
        &base,
        "qa",
        &["db", "create", &name, "--server", "qa", "--json"],
    );
    assert_success(&created);
    assert_eq!(
        json(&created),
        serde_json::json!({ "database": name, "created": true, "server": "qa" })
    );

    let dropped = on_server(
        &sandbox,
        &base,
        "qa",
        &["db", "drop", &name, "--server", "qa", "--yes", "--json"],
    );
    assert_success(&dropped);
    assert_eq!(
        json(&dropped),
        serde_json::json!({ "database": name, "dropped": true, "server": "qa" })
    );
}

#[test]
fn db_create_and_drop_quote_the_name() {
    let Some(base) = database_url_or_skip("db_create_and_drop_quote_the_name") else {
        return;
    };
    let admin = url_for(&base, "postgres");
    let name = format!("{} we\"ird; name", unique_name("quoted"));
    let sandbox = Sandbox::new();

    assert_success(&sandbox.run(&["db", "create", &name, "--database-url", &admin]));
    assert!(database_exists(&base, &name));
    assert_success(&sandbox.run(&["db", "drop", &name, "--database-url", &admin, "--yes"]));
    assert!(!database_exists(&base, &name));
}

#[test]
fn db_create_warns_when_it_falls_back_to_database_url() {
    let Some(base) = database_url_or_skip("db_create_warns_when_it_falls_back_to_database_url")
    else {
        return;
    };
    let name = unique_name("fallback");
    let sandbox = Sandbox::new();

    let output = sandbox
        .rok(&["db", "create", &name])
        .env("DATABASE_URL", url_for(&base, "postgres"))
        .output()
        .expect("spawn rok");
    assert_success(&output);
    assert!(
        stderr(&output).starts_with("warning: no --server or --database-url"),
        "{}",
        stderr(&output)
    );
    assert_success(&sandbox.run(&[
        "db",
        "drop",
        &name,
        "--database-url",
        &url_for(&base, "postgres"),
        "--yes",
    ]));
}

#[test]
fn db_precedence_database_url() {
    let Some(base) = database_url_or_skip("db_precedence_database_url") else {
        return;
    };
    let db = TestDb::create(&base, "prec_url");
    // The server points nowhere: success proves it was not used.
    let sandbox = Sandbox::new();
    sandbox.add_server("elsewhere", NOWHERE);

    let output = sandbox
        .rok(&[
            "db",
            "query",
            "SELECT current_database()",
            "--database-url",
            &db.url,
            "--server",
            "elsewhere",
            "--json",
        ])
        .env(password_var("elsewhere"), "unused")
        .env("DATABASE_URL", NOWHERE)
        .output()
        .expect("spawn rok");
    assert_eq!(current_database(&output), db.name);
}

#[test]
fn db_precedence_server() {
    let Some(base) = database_url_or_skip("db_precedence_server") else {
        return;
    };
    let db = TestDb::create(&base, "prec_server");
    let sandbox = Sandbox::new();
    sandbox.add_server("qa", &db.url);

    // DATABASE_URL points nowhere: success proves the server was used.
    let output = sandbox
        .rok(&[
            "db",
            "query",
            "SELECT current_database()",
            "--server",
            "qa",
            "--json",
        ])
        .env(password_var("qa"), base.password().unwrap_or_default())
        .env("DATABASE_URL", NOWHERE)
        .output()
        .expect("spawn rok");
    assert_eq!(current_database(&output), db.name);
}

#[test]
fn db_precedence_env() {
    let Some(base) = database_url_or_skip("db_precedence_env") else {
        return;
    };
    let db = TestDb::create(&base, "prec_env");

    let output = Sandbox::new()
        .rok(&["db", "query", "SELECT current_database()", "--json"])
        .env("DATABASE_URL", &db.url)
        .output()
        .expect("spawn rok");
    assert_eq!(current_database(&output), db.name);
}
