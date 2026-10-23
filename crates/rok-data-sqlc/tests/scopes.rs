//! Named scopes of the generated models (spec D7), through the CLI: the
//! `[generate.scopes]` entries become the models' scope attributes. The
//! scopes' behavior — the composition, the `Scope` trait — is proven in
//! `rok-data`'s facade tests.

use std::fs;
use std::path::Path;

use anyhow::Result;
use tempfile::TempDir;

fn run(args: &[&str]) -> Result<()> {
    let mut full: Vec<String> = vec!["rok-data-sqlc".to_owned()];
    full.extend(args.iter().map(|arg| arg.to_string()));
    rok_data_sqlc::cli::run_from(full)
}

/// Generates `schema` into `out/` of a temp dir, with `extra_toml` in its
/// `rok-data-sqlc.toml`.
fn generate(schema: &str, extra_toml: &str) -> (TempDir, Result<()>) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("schema.sql"), schema).unwrap();
    let config = dir.path().join("rok-data-sqlc.toml");
    fs::write(
        &config,
        format!("schema = \"schema.sql\"\noutput = \"out\"\n{extra_toml}"),
    )
    .unwrap();
    let result = run(&["g", "-c", config.to_str().unwrap()]);
    (dir, result)
}

fn models(dir: &Path, module: &str) -> Option<String> {
    fs::read_to_string(dir.join("out").join(module).join("models.rs")).ok()
}

const USERS_SCHEMA: &str = "CREATE TABLE users (
    id BIGINT PRIMARY KEY,
    name TEXT NOT NULL
);
";

/// A configured scope lands on the model, naming the path the
/// configuration gives.
#[test]
fn a_configured_scope_becomes_the_models_scope_attribute() -> Result<()> {
    let (dir, result) = generate(
        USERS_SCHEMA,
        "[scopes.users]\nfamous = \"crate::scopes::users::famous\"\n",
    );
    result?;
    let users = models(dir.path(), "users").expect("users generates");
    assert!(
        users.contains("#[rok(scopes(famous = crate::scopes::users::famous))]"),
        "{users}"
    );
    // The attribute sits on the row struct, after the other containers.
    assert!(
        users.contains(
            "#[rok(table = \"users\")]\n#[rok(scopes(famous = crate::scopes::users::famous))]"
        ),
        "{users}"
    );
    Ok(())
}

/// Two scopes render as two attributes; a table without entries carries
/// none.
#[test]
fn every_declared_scope_gets_its_own_attribute() -> Result<()> {
    let (dir, result) = generate(
        USERS_SCHEMA,
        "[scopes.users]\n\
         famous = \"crate::scopes::users::famous\"\n\
         adults = \"crate::scopes::users::adults\"\n",
    );
    result?;
    let users = models(dir.path(), "users").expect("users generates");
    assert!(
        users.contains("#[rok(scopes(adults = crate::scopes::users::adults))]"),
        "{users}"
    );
    assert!(
        users.contains("#[rok(scopes(famous = crate::scopes::users::famous))]"),
        "{users}"
    );
    assert_eq!(
        users
            .lines()
            .filter(|line| line.contains("#[rok(scopes("))
            .count(),
        2,
        "{users}"
    );
    Ok(())
}

fn failure(schema: &str, extra_toml: &str) -> String {
    let (dir, result) = generate(schema, extra_toml);
    let message = format!("{:#}", result.expect_err("generation fails"));
    assert!(!dir.path().join("out").exists(), "{message}");
    message
}

/// An entry that names no table, a scope name that is not a snake_case
/// Rust identifier, or a function path that cannot be one fails with the
/// configuration entry named.
#[test]
fn bad_entries_fail_naming_the_config_entry() {
    let message = failure(
        USERS_SCHEMA,
        "[scopes.notes]\nfamous = \"crate::scopes::notes::famous\"\n",
    );
    assert!(
        message.contains("`scopes.notes` in the configuration names no table of the schema"),
        "{message}"
    );

    let message = failure(
        USERS_SCHEMA,
        "[scopes.users]\nFamous = \"crate::scopes::users::famous\"\n",
    );
    assert!(
        message
            .contains("`scopes.users.Famous` in the configuration names the scope `Famous`, which is not a snake_case Rust identifier"),
        "{message}"
    );

    let message = failure(USERS_SCHEMA, "[scopes.users]\nfamous = \"\"\n");
    assert!(
        message.contains("`scopes.users.famous` in the configuration names the scope function ``, which is not a Rust path"),
        "{message}"
    );

    let message = failure(
        USERS_SCHEMA,
        "[scopes.users]\nfamous = \"crate:: scopes:: famous\"\n",
    );
    assert!(
        message
            .contains("`scopes.users.famous` in the configuration names the scope function `crate:: scopes:: famous`, which is not a Rust path"),
        "{message}"
    );
}
