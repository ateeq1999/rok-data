//! Binary-level tests for `rok server add|list|remove`.
//!
//! Spawns the `rok` binary with a temporary config directory, never touching
//! the real keychain or user config.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

/// Runs `rok server ...` with a custom config dir.
fn run_server(temp_dir: &TempDir, args: &[&str]) -> Command {
    let config_dir = temp_dir.path().join("config");
    fs::create_dir_all(&config_dir).expect("create config dir");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rok"));
    cmd.arg("server")
        .args(args)
        // ROK_CONFIG_DIR overrides the platform config directory (see config_dir())
        .env("ROK_CONFIG_DIR", &config_dir)
        // Ensure no real keychain is probed
        .env("ROK_TEST_NO_KEYCHAIN", "1")
        .env_remove("ROK_SERVER_LOCAL_PASSWORD")
        .env_remove("ROK_SERVER_STAGING_PASSWORD")
        .env_remove("ROK_SERVER_CI_PASSWORD")
        .env_remove("ROK_SERVER_TEST_PASSWORD");
    cmd
}

/// Runs `rok server ...` and returns the output.
fn run_server_output(temp_dir: &TempDir, args: &[&str]) -> std::process::Output {
    run_server(temp_dir, args)
        .output()
        .expect("failed to spawn rok binary")
}

/// Asserts the command succeeded.
fn assert_success(output: &std::process::Output) {
    if !output.status.success() {
        panic!(
            "command failed (exit code: {}):\nstdout: {}\nstderr: {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Asserts the command failed with the given message in stderr.
fn assert_failure(output: &std::process::Output, expected_msg: &str) {
    assert!(
        !output.status.success(),
        "expected failure but command succeeded"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected_msg),
        "expected '{}' in stderr:\n{}",
        expected_msg,
        stderr
    );
}

/// Gets the servers.toml path for a temp dir by replicating config_dir() logic.
fn servers_toml_path(temp_dir: &TempDir) -> PathBuf {
    let config_dir = temp_dir.path().join("config");
    // In tests we set ROK_CONFIG_DIR to this config_dir
    config_dir.join("servers.toml")
}

#[test]
fn server_add_with_url_stores_password_in_keychain() {
    let temp_dir = TempDir::new().expect("temp dir");

    // Add server with URL containing password
    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "local",
            "--url",
            "postgres://postgres:secret@localhost:5432/postgres",
        ],
    );
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Server 'local' added"));
    // Keychain might be available or not; accept either message
    assert!(
        stdout.contains("Password stored in OS keychain")
            || stdout.contains("password from ROK_SERVER_LOCAL_PASSWORD"),
        "unexpected stdout: {}",
        stdout
    );

    // Verify servers.toml was created
    let servers_path = servers_toml_path(&temp_dir);
    assert!(
        servers_path.exists(),
        "servers.toml not found at {}",
        servers_path.display()
    );
    let content = fs::read_to_string(&servers_path).expect("read servers.toml");
    assert!(content.contains("name = \"local\""));
    assert!(content.contains("host = \"localhost\""));
    assert!(content.contains("port = 5432"));
    assert!(content.contains("user = \"postgres\""));
    assert!(content.contains("database = \"postgres\""));
    // Password should NOT be in servers.toml
    assert!(!content.contains("secret"));
    assert!(!content.contains("password"));
}

#[test]
fn server_add_with_discrete_flags_and_password_prompt() {
    let temp_dir = TempDir::new().expect("temp dir");

    // We can't easily test interactive password prompt in automated tests,
    // so we test the validation that either URL or flags+password are provided.
    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "staging",
            "--host",
            "staging-db.example.com",
            "--user",
            "rok",
            "--port",
            "5432",
            "--database",
            "postgres",
            "--sslmode",
            "require",
        ],
    );
    // Should fail because no password provided
    assert_failure(&output, "password required");
}

#[test]
fn server_add_without_url_or_flags_fails() {
    let temp_dir = TempDir::new().expect("temp dir");

    let output = run_server_output(&temp_dir, &["add", "test"]);
    assert_failure(&output, "--host is required");
}

#[test]
fn server_add_duplicate_name_errors() {
    let temp_dir = TempDir::new().expect("temp dir");

    // Add first server
    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "local",
            "--url",
            "postgres://postgres:pass1@localhost:5432/postgres",
        ],
    );
    assert_success(&output);

    // Try to add second server with same name
    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "local",
            "--url",
            "postgres://postgres:pass2@localhost:5432/postgres",
        ],
    );
    assert_failure(&output, "server 'local' already exists");
}

#[test]
fn server_list_shows_registered_servers() {
    let temp_dir = TempDir::new().expect("temp dir");

    // Add a server first
    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "local",
            "--url",
            "postgres://postgres:secret@localhost:5432/postgres",
        ],
    );
    assert_success(&output);

    // List servers
    let output = run_server_output(&temp_dir, &["list"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("local"));
    assert!(stdout.contains("localhost"));
    assert!(stdout.contains("5432"));
    assert!(stdout.contains("postgres"));
    // Password status should show keychain ✓ (or env if keychain unavailable)
    assert!(
        stdout.contains("✓") || stdout.contains("✗"),
        "unexpected stdout: {}",
        stdout
    );
}

#[test]
fn server_list_empty_shows_no_servers() {
    let temp_dir = TempDir::new().expect("temp dir");

    let output = run_server_output(&temp_dir, &["list"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No servers registered"));
}

#[test]
fn server_remove_deletes_server_and_password() {
    let temp_dir = TempDir::new().expect("temp dir");

    // Add a server first
    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "local",
            "--url",
            "postgres://postgres:secret@localhost:5432/postgres",
        ],
    );
    assert_success(&output);

    // Remove with --yes to skip confirmation
    let output = run_server_output(&temp_dir, &["remove", "local", "--yes"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Server 'local' removed"));
    // Keychain might be available or not
    assert!(
        stdout.contains("Password deleted from OS keychain")
            || stdout.contains("Unset ROK_SERVER_LOCAL_PASSWORD"),
        "unexpected stdout: {}",
        stdout
    );

    // List should now be empty
    let output = run_server_output(&temp_dir, &["list"]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No servers registered"));
}

#[test]
fn server_remove_unknown_name_errors() {
    let temp_dir = TempDir::new().expect("temp dir");

    let output = run_server_output(&temp_dir, &["remove", "unknown", "--yes"]);
    assert_failure(&output, "server 'unknown' not found");
}

#[test]
fn server_remove_requires_confirmation_without_yes() {
    let temp_dir = TempDir::new().expect("temp dir");

    // Add a server first
    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "local",
            "--url",
            "postgres://postgres:secret@localhost:5432/postgres",
        ],
    );
    assert_success(&output);

    // Try to remove without --yes (should fail in non-interactive test)
    let mut cmd = run_server(&temp_dir, &["remove", "local"]);
    // Don't provide input, so confirmation should fail
    let output = cmd.output().expect("failed to spawn rok binary");

    // The command should fail because stdin is empty and confirmation doesn't match
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("confirmation failed") || stderr.contains("server name does not match")
    );
}

#[test]
fn server_add_list_remove_json_output() {
    let temp_dir = TempDir::new().expect("temp dir");

    // Add with --json
    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "local",
            "--url",
            "postgres://postgres:secret@localhost:5432/postgres",
            "--json",
        ],
    );
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"name\":\"local\""));
    assert!(stdout.contains("\"added\":true"));
    assert!(stdout.contains("\"password_stored_in_keychain\""));

    // List with --json
    let output = run_server_output(&temp_dir, &["list", "--json"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"servers\""));
    assert!(stdout.contains("\"name\":\"local\""));
    assert!(stdout.contains("\"host\":\"localhost\""));
    assert!(stdout.contains("\"port\":5432"));
    assert!(stdout.contains("\"password\""));
    assert!(stdout.contains("\"present\":true"));

    // Remove with --json
    let output = run_server_output(&temp_dir, &["remove", "local", "--yes", "--json"]);
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"name\":\"local\""));
    assert!(stdout.contains("\"removed\":true"));
    assert!(stdout.contains("\"password_deleted_from_keychain\""));
}

#[test]
fn server_add_with_sslmode_in_url() {
    let temp_dir = TempDir::new().expect("temp dir");

    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "staging",
            "--url",
            "postgres://rok:pass@staging-db.example.com:5432/postgres?sslmode=require",
        ],
    );
    assert_success(&output);

    let servers_path = servers_toml_path(&temp_dir);
    let content = fs::read_to_string(&servers_path).expect("read servers.toml");
    assert!(content.contains("sslmode = \"require\""));
}

#[test]
fn server_add_without_sslmode_defaults_to_none() {
    let temp_dir = TempDir::new().expect("temp dir");

    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "local",
            "--url",
            "postgres://postgres:pass@localhost:5432/postgres",
        ],
    );
    assert_success(&output);

    let servers_path = servers_toml_path(&temp_dir);
    let content = fs::read_to_string(&servers_path).expect("read servers.toml");
    // sslmode should not be present when not specified
    assert!(!content.contains("sslmode"));
}

#[test]
fn server_add_postgresql_scheme_works() {
    let temp_dir = TempDir::new().expect("temp dir");

    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "local",
            "--url",
            "postgresql://postgres:pass@localhost:5432/postgres",
        ],
    );
    assert_success(&output);

    let servers_path = servers_toml_path(&temp_dir);
    let content = fs::read_to_string(&servers_path).expect("read servers.toml");
    assert!(content.contains("name = \"local\""));
}

#[test]
fn server_add_invalid_scheme_fails() {
    let temp_dir = TempDir::new().expect("temp dir");

    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "local",
            "--url",
            "mysql://postgres:pass@localhost:5432/postgres",
        ],
    );
    assert_failure(&output, "URL scheme must be postgres:// or postgresql://");
}

#[test]
fn server_add_missing_host_in_url_fails() {
    let temp_dir = TempDir::new().expect("temp dir");

    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "local",
            "--url",
            "postgres://postgres:pass@/postgres",
        ],
    );
    assert_failure(&output, "empty host");
}

#[test]
fn server_add_missing_user_in_url_fails() {
    let temp_dir = TempDir::new().expect("temp dir");

    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "local",
            "--url",
            "postgres://:pass@localhost:5432/postgres",
        ],
    );
    assert_failure(&output, "URL must include a username");
}

#[test]
fn server_add_discrete_flags_require_host_and_user() {
    let temp_dir = TempDir::new().expect("temp dir");

    // Missing --host (fails with host required)
    let output = run_server_output(
        &temp_dir,
        &["add", "test", "--user", "postgres", "--password-prompt"],
    );
    assert_failure(&output, "--host is required");

    // Missing --user (fails with user required)
    let output = run_server_output(
        &temp_dir,
        &["add", "test", "--host", "localhost", "--password-prompt"],
    );
    assert_failure(&output, "--user <USER>");
}

#[test]
fn server_add_url_conflicts_with_discrete_flags() {
    let temp_dir = TempDir::new().expect("temp dir");

    // clap should reject this at parse time
    let output = run_server_output(
        &temp_dir,
        &[
            "add",
            "test",
            "--url",
            "postgres://postgres:pass@localhost:5432/postgres",
            "--host",
            "other",
        ],
    );
    assert_failure(&output, "cannot be used with");
}
