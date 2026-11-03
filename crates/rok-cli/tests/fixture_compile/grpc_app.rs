//! `tests/grpc-app`: an application generated with `[api] framework =
//! "grpc"` (engine ADR 0028): the axum fixture's features and a table of
//! every type a message carries, served as tonic services over the same
//! service traits. Its `build.rs` compiles the generated protos with
//! protox, so no `protoc` is needed.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use anyhow::{Context, Result, ensure};

use crate::common::{self, cargo};

/// The gRPC fixture crate, `tests/grpc-app`.
pub(crate) fn grpc_app_dir() -> PathBuf {
    common::app_dir("grpc-app")
}

/// Every test regenerates into the gRPC fixture crate, so they run one at a
/// time.
static GRPC_APP: Mutex<()> = Mutex::new(());

/// Generates the fixture's data layer, the shared tools, the API features
/// and the protos, and returns the generated Rust files.
fn generate_grpc_app() -> Result<Vec<PathBuf>> {
    let mut config = rok_data_sqlc::Config::default();
    config.schema = vec![grpc_app_dir().join("migrations")];
    config.output = grpc_app_dir().join("src/db");
    config.force = true;
    config.derive = vec![
        rok_data_sqlc::Derive::Serde,
        rok_data_sqlc::Derive::Validate,
    ];
    // gRPC reads no HTTP body: the data layer needs no extractor.
    config.conventions.primary_key_ids = Some(rok_data_sqlc::IdGeneration::UuidV7);
    // The conventions: the notes are tenant-scoped, versioned and
    // soft-deleted.
    config.conventions.tenant_column = Some("tenant_id".into());
    config.conventions.version_column = Some("revision".into());
    config.conventions.soft_delete_column = Some("deleted_at".into());
    let mut api = rok_data_sqlc::ApiConfig::new(grpc_app_dir().join("src/features"));
    api.framework = rok_data_sqlc::ApiFramework::Grpc;
    // gRPC serves no HTTP route to document.
    api.openapi = false;
    api.search_endpoint = true;
    api.search
        .insert("notes".into(), vec!["title".into(), "body".into()]);
    // The labels and the gadgets are public, the accounts admin only, and
    // the notes keep the default (`user`), owned through `owner_id`.
    api.access
        .insert("labels".into(), rok_data_sqlc::ApiAccess::Public);
    api.access
        .insert("gadgets".into(), rok_data_sqlc::ApiAccess::Public);
    api.access
        .insert("accounts".into(), rok_data_sqlc::ApiAccess::Admin);
    api.hidden
        .insert("accounts".into(), vec!["password_hash".into()]);
    // An account's email is set on create and never changed: the
    // `UpdateAccountRequest` has no field for it.
    api.immutable
        .insert("accounts".into(), vec!["email".into()]);
    api.owner.insert("notes".into(), "owner_id".to_owned());
    config.api = Some(api);

    let report = rok_data_sqlc::generate(&config)?;
    let files = report.written.into_iter().chain(report.unchanged);
    Ok(files
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect())
}

/// The gRPC fixture passes the lint gate (engine ADR 0013), its protos
/// compiled by its `build.rs`: `deny(missing_docs)`, the selected pedantic
/// lints, and `-D warnings`, with `unwrap`, `expect` and `panic` denied.
#[test]
fn grpc_app_passes_the_lint_gate() -> Result<()> {
    let _guard = GRPC_APP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(reason) = common::generated_code_skip_reason(&grpc_app_dir()) {
        eprintln!("{reason}");
        return Ok(());
    }
    generate_grpc_app()?;

    let status = cargo(
        &grpc_app_dir(),
        &["clippy", "--quiet", "--all-targets", "--", "-Dwarnings"],
    )
    .status()
    .context("failed to run cargo clippy in tests/grpc-app")?;
    ensure!(status.success(), "the gRPC fixture failed the lint gate");
    Ok(())
}

/// The gRPC fixture's tests: the services called over an in-process
/// channel. The calls that reach the database run with
/// `ROK_DATA_SQLC_TEST_DATABASE_URL` set and skip, with a note, without it.
#[test]
fn grpc_app_passes_its_tests() -> Result<()> {
    let _guard = GRPC_APP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(reason) = common::generated_code_skip_reason(&grpc_app_dir()) {
        eprintln!("{reason}");
        return Ok(());
    }
    generate_grpc_app()?;

    let status = cargo(&grpc_app_dir(), &["test", "--quiet"])
        .status()
        .context("failed to run cargo test in tests/grpc-app")?;
    ensure!(status.success(), "the gRPC fixture failed its tests");
    Ok(())
}

/// The generated code obeys the architecture it generates: the gRPC
/// fixture passes `rok sqlc check-arch` with no warning, reading the
/// fixture's `rok-data-sqlc.toml`.
#[test]
fn grpc_app_passes_check_arch() -> Result<()> {
    let _guard = GRPC_APP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    generate_grpc_app()?;

    let output = Command::new(env!("CARGO_BIN_EXE_rok-data"))
        .arg("sqlc")
        .arg("check-arch")
        .current_dir(grpc_app_dir())
        .output()
        .context("failed to run rok sqlc check-arch")?;
    ensure!(
        output.status.success() && output.stdout.is_empty() && output.stderr.is_empty(),
        "the gRPC fixture fails check-arch:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn grpc_app_generated_code_is_rustfmt_clean() -> Result<()> {
    let _guard = GRPC_APP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let files = generate_grpc_app()?;
    ensure!(!files.is_empty(), "the gRPC fixture generated no files");

    let output = Command::new("rustfmt")
        .args(["--edition", "2024", "--check"])
        .args(&files)
        .output()
        .context("failed to run rustfmt (install it with `rustup component add rustfmt`)")?;
    ensure!(
        output.status.success(),
        "the generated gRPC code is not rustfmt-clean:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}
