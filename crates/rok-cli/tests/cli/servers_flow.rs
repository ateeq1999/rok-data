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
