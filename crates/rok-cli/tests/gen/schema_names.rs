//! `rok gen` and the OpenAPI document's schema names: utoipa keeps one
//! schema per component name, so two `ToSchema` types sharing one would
//! silently replace each other in the contract and the Clients. The run
//! fails naming both instead.

use super::*;

/// A project serving `users` under `src/features`, with `extra` written
/// as the hand-written `src/dto.rs`.
fn api_project_with(extra: &str) -> Result<TempDir> {
    let dir = project(SCHEMA)?;
    fs::write(
        dir.path().join("rok.toml"),
        "[generate]\nschema = \"migrations/\"\nderive = [\"serde\", \"validate\"]\n\
         extractors = [\"actix_web\"]\n\n[api]\noutput = \"src/features\"\n",
    )?;
    fs::create_dir_all(dir.path().join("src"))?;
    fs::write(dir.path().join("src/dto.rs"), extra)?;
    Ok(dir)
}

/// A hand-written DTO named like the generated `UserResponse`.
const CLASHING: &str = "/// Hand-written.\n#[derive(serde::Serialize, utoipa::ToSchema)]\n\
                        pub struct UserResponse {\n    pub id: uuid::Uuid,\n}\n";

/// Both runs fail, naming the two files and lines and the two fixes.
#[test]
fn two_schemas_with_one_name_fail_gen_and_check() -> Result<()> {
    let dir = api_project_with(CLASHING)?;
    let output = run_in(dir.path(), &["gen"])?;
    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    let stderr = stderr(&output);
    assert!(
        stderr.contains("two `ToSchema` types are named `UserResponse` in the OpenAPI document"),
        "{stderr}"
    );
    assert!(stderr.contains("src/dto.rs:3"), "{stderr}");
    assert!(stderr.contains("src/features/users/dto.rs:"), "{stderr}");
    assert!(stderr.contains("#[schema(as = "), "{stderr}");
    assert!(
        stderr.contains("steps:\n  done     the data layer\n  failed   the schema names\n"),
        "{stderr}"
    );

    let output = run_in(dir.path(), &["gen", "check"])?;
    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    let stderr = self::stderr(&output);
    assert!(
        stderr.contains("two `ToSchema` types are named `UserResponse`"),
        "{stderr}"
    );
    Ok(())
}

/// `#[schema(as = …)]` gives the hand-written type a name of its own.
#[test]
fn schema_as_resolves_the_clash() -> Result<()> {
    let renamed = CLASHING.replace(
        "pub struct",
        "#[schema(as = admin::UserResponse)]\npub struct",
    );
    let dir = api_project_with(&renamed)?;
    assert_success(&run_in(dir.path(), &["gen"])?);
    assert_success(&run_in(dir.path(), &["gen", "check"])?);
    Ok(())
}
