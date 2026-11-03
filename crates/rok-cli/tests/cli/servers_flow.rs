//! `rok server` end to end with no OS keychain (`ROK_KEYCHAIN=off`): the
//! registry round trip, and passwords that come only from
//! `ROK_SERVER_<NAME>_PASSWORD` (ADR 0028).

use crate::db_support::{Sandbox, assert_error, assert_success, json, stderr, stdout};

#[test]
fn server_add_list_remove_roundtrip() {
    let sandbox = Sandbox::new();
    sandbox.add_server(
        "alpha",
        "postgres://ann:pw@alpha.example.com:6543/admin?sslmode=require",
    );
    sandbox.add_server("beta", "postgresql://bob:pw@beta.example.com/postgres");

    let listed = json(&sandbox.run(&["server", "list", "--json"]));
    assert_eq!(
        listed,
        serde_json::json!({ "servers": [
            { "name": "alpha", "host": "alpha.example.com", "port": 6543, "user": "ann",
              "database": "admin", "sslmode": "require",
              "password": { "source": "none", "present": false } },
            { "name": "beta", "host": "beta.example.com", "port": 5432, "user": "bob",
              "database": "postgres", "sslmode": null,
              "password": { "source": "none", "present": false } },
        ] })
    );

    assert_success(&sandbox.run(&["server", "remove", "alpha", "--yes"]));
    let output = sandbox.run(&["server", "remove", "beta", "--yes", "--json"]);
    assert_success(&output);
    assert_eq!(
        json(&output),
        serde_json::json!({ "name": "beta", "removed": true, "password_deleted_from_keychain": false })
    );

    let output = sandbox.run(&["server", "list"]);
    assert_success(&output);
    assert_eq!(stdout(&output), "No servers registered.\n");
    assert_eq!(
        json(&sandbox.run(&["server", "list", "--json"])),
        serde_json::json!({ "servers": [] })
    );
}

#[test]
fn server_add_duplicate_error() {
    let sandbox = Sandbox::new();
    sandbox.add_server("dup", "postgres://u:p@h/postgres");
    let output = sandbox.run(&[
        "server",
        "add",
        "dup",
        "--url",
        "postgres://u:p@other/postgres",
    ]);
    assert_error(&output, "server 'dup' already exists");
}

#[test]
fn server_remove_unknown_error() {
    assert_error(
        &Sandbox::new().run(&["server", "remove", "ghost", "--yes"]),
        "server 'ghost' not found",
    );
}

#[test]
fn server_keychain_fallback() {
    let sandbox = Sandbox::new();

    // With the keychain off, add says where the password must come from.
    let output = sandbox.run(&[
        "server",
        "add",
        "ci-box",
        "--url",
        "postgres://ci:pw@ci.example.com/postgres",
    ]);
    assert_success(&output);
    assert!(
        stderr(&output).contains("ROK_SERVER_CI_BOX_PASSWORD"),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        stdout(&output),
        "Server 'ci-box' added (password from ROK_SERVER_CI_BOX_PASSWORD).\n"
    );
    // No variable set: the password is reported missing.
    let output = sandbox.run(&["server", "list"]);
    assert!(stdout(&output).contains("missing ✗"), "{}", stdout(&output));

    // With the variable set, list reports it as the source, never its value.
    let output = sandbox
        .rok(&["server", "list"])
        .env("ROK_SERVER_CI_BOX_PASSWORD", "from-env-pw")
        .output()
        .expect("spawn rok");
    assert_success(&output);
    assert!(
        stdout(&output).contains("env ROK_SERVER_CI_BOX_PASSWORD ✓"),
        "{}",
        stdout(&output)
    );
    assert!(!stdout(&output).contains("from-env-pw"));

    // Remove cannot delete a keychain entry it never wrote: it says so.
    let output = sandbox.run(&["server", "remove", "ci-box", "--yes"]);
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "Server 'ci-box' removed. Unset ROK_SERVER_CI_BOX_PASSWORD if no longer needed.\n"
    );
}

#[test]
fn server_add_without_a_password_uses_the_variable() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&[
        "server",
        "add",
        "ci",
        "--url",
        "postgres://ci@ci.example.com/postgres",
    ]);
    assert_success(&output);
    assert!(
        stderr(&output).contains("ROK_SERVER_CI_PASSWORD"),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        stdout(&output),
        "Server 'ci' added (password from ROK_SERVER_CI_PASSWORD).\n"
    );

    let output = sandbox
        .rok(&["server", "list", "--json"])
        .env("ROK_SERVER_CI_PASSWORD", "x")
        .output()
        .expect("spawn rok");
    assert_eq!(
        json(&output)["servers"][0]["password"],
        serde_json::json!({ "source": "env", "present": true })
    );
}

#[test]
fn server_remove_prompt_stays_off_stdout() {
    use std::io::Write;
    use std::process::Stdio;

    let sandbox = Sandbox::new();
    sandbox.add_server("gone", "postgres://u:p@h/postgres");
    let mut child = sandbox
        .rok(&["server", "remove", "gone"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rok");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"gone\n")
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait for rok");
    assert_success(&output);
    assert!(stderr(&output).contains("Type the server name to confirm: "));
    assert!(!stdout(&output).contains("confirm"), "{}", stdout(&output));
}

/// `server update` changes only the settings given; the rest keep their
/// values, and the change is what `list` shows.
#[test]
fn server_update_changes_only_the_settings_given() {
    let sandbox = Sandbox::new();
    sandbox.add_server(
        "local",
        "postgres://ann:pw@db.example.com:6543/admin?sslmode=require",
    );

    let output = sandbox.run(&[
        "server",
        "update",
        "local",
        "--host",
        "localhost",
        "--port",
        "5432",
        "--json",
    ]);
    assert_success(&output);
    assert_eq!(
        json(&output),
        serde_json::json!({ "name": "local", "updated": ["host", "port"],
                            "password_stored_in_keychain": null })
    );
    assert_eq!(
        json(&sandbox.run(&["server", "list", "--json"]))["servers"][0],
        serde_json::json!({ "name": "local", "host": "localhost", "port": 5432, "user": "ann",
                            "database": "admin", "sslmode": "require",
                            "password": { "source": "none", "present": false } })
    );
}

/// `--url` replaces the connection whole, and its password goes to the
/// keychain (off here, so it says the variable supplies it).
#[test]
fn server_update_with_a_url_replaces_the_connection() {
    let sandbox = Sandbox::new();
    sandbox.add_server(
        "local",
        "postgres://ann:pw@old.example.com/admin?sslmode=require",
    );

    let output = sandbox.run(&[
        "server",
        "update",
        "local",
        "--url",
        "postgres://bob:secret@new.example.com:5433/app",
        "--json",
    ]);
    assert_success(&output);
    assert_eq!(
        json(&output)["password_stored_in_keychain"],
        serde_json::json!(false)
    );
    assert_eq!(
        json(&sandbox.run(&["server", "list", "--json"]))["servers"][0],
        serde_json::json!({ "name": "local", "host": "new.example.com", "port": 5433, "user": "bob",
                            "database": "app", "sslmode": null,
                            "password": { "source": "none", "present": false } })
    );
}

/// `--password-prompt` alone changes no setting: it asks, then stores.
#[test]
fn server_update_password_prompt_keeps_the_settings() {
    use std::io::Write;
    use std::process::Stdio;

    let sandbox = Sandbox::new();
    sandbox.add_server("local", "postgres://ann@db.example.com/admin");
    let mut child = sandbox
        .rok(&["server", "update", "local", "--password-prompt"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rok server update");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(b"new-secret\n")
        .expect("write the password");
    let output = child.wait_with_output().expect("wait for rok");
    assert_success(&output);
    assert!(
        stderr(&output).contains("Enter password: "),
        "{}",
        stderr(&output)
    );
    assert!(
        stdout(&output)
            .contains("Server 'local' updated: password from ROK_SERVER_LOCAL_PASSWORD."),
        "{}",
        stdout(&output)
    );
    assert_eq!(
        json(&sandbox.run(&["server", "list", "--json"]))["servers"][0]["host"],
        serde_json::json!("db.example.com")
    );
}

#[test]
fn server_update_unknown_server_names_it() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&["server", "update", "ghost", "--port", "5433"]);
    assert_error(
        &output,
        "server 'ghost' not found; `rok server add ghost` registers it",
    );
}

#[test]
fn server_update_without_a_change_is_a_usage_error() {
    let sandbox = Sandbox::new();
    sandbox.add_server("local", "postgres://ann@db.example.com/admin");
    let output = sandbox.run(&["server", "update", "local"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
}
