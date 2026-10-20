//! `tests/axum-app`: an application laid out by `rok sqlc init` and
//! generated with `api.output` for axum (ADR 0016): the same feature set
//! as the actix fixture, on the other backend.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use anyhow::{Context, Result, ensure};

use crate::common::{self, cargo};

/// The axum API fixture crate, `tests/axum-app` (ADR 0016): an application
/// laid out by `rok sqlc init` and generated for axum.
pub(crate) fn axum_app_dir() -> PathBuf {
    common::app_dir("axum-app")
}

/// Every test regenerates into the axum fixture crate, so they run one at a
/// time.
static AXUM_APP: Mutex<()> = Mutex::new(());

/// Generates the fixture's data layer, the shared tools and the API
/// features, and returns the generated Rust files.
fn generate_axum_app() -> Result<Vec<PathBuf>> {
    let mut config = rok_data_sqlc::Config::default();
    config.schema = vec![axum_app_dir().join("migrations")];
    config.output = axum_app_dir().join("src/db");
    config.force = true;
    config.derive = vec![
        rok_data_sqlc::Derive::Serde,
        rok_data_sqlc::Derive::Validate,
    ];
    config.extractors = vec![rok_data_sqlc::Extractor::Axum];
    config.conventions.primary_key_ids = Some(rok_data_sqlc::IdGeneration::UuidV7);
    // The conventions: the notes are tenant-scoped, versioned and
    // soft-deleted.
    config.conventions.tenant_column = Some("tenant_id".into());
    config.conventions.version_column = Some("revision".into());
    config.conventions.soft_delete_column = Some("deleted_at".into());
    let mut api = rok_data_sqlc::ApiConfig::new(axum_app_dir().join("src/features"));
    api.framework = rok_data_sqlc::ApiFramework::Axum;
    api.search_endpoint = true;
    api.search
        .insert("notes".into(), vec!["title".into(), "body".into()]);
    api.sort.insert("notes".into(), vec!["title".into()]);
    // The labels are public, the accounts admin only, and the notes keep
    // the default (`user`), owned through `owner_id`.
    api.access
        .insert("labels".into(), rok_data_sqlc::ApiAccess::Public);
    api.access
        .insert("accounts".into(), rok_data_sqlc::ApiAccess::Admin);
    api.hidden
        .insert("accounts".into(), vec!["password_hash".into()]);
    api.owner.insert("notes".into(), "owner_id".to_owned());
    config.api = Some(api);

    let report = rok_data_sqlc::generate(&config)?;
    // The Rust files: `README.md` is generated too.
    let files = report.written.into_iter().chain(report.unchanged);
    Ok(files
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect())
}

/// The axum fixture passes the lint gate (ADR 0013): `deny(missing_docs)`,
/// the selected pedantic lints, and `-D warnings`, with `unwrap`, `expect`
/// and `panic` denied.
#[test]
fn axum_app_passes_the_lint_gate() -> Result<()> {
    let _guard = AXUM_APP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(reason) = common::generated_code_skip_reason(&axum_app_dir()) {
        eprintln!("{reason}");
        return Ok(());
    }
    generate_axum_app()?;

    let status = cargo(
        &axum_app_dir(),
        &["clippy", "--quiet", "--all-targets", "--", "-Dwarnings"],
    )
    .status()
    .context("failed to run cargo clippy in tests/axum-app")?;
    ensure!(
        status.success(),
        "the axum API fixture failed the lint gate"
    );
    Ok(())
}

/// The axum fixture's tests: the document, the routes and the answers that
/// stop before the data layer.
#[test]
fn axum_app_passes_its_tests() -> Result<()> {
    let _guard = AXUM_APP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(reason) = common::generated_code_skip_reason(&axum_app_dir()) {
        eprintln!("{reason}");
        return Ok(());
    }
    generate_axum_app()?;

    let status = cargo(&axum_app_dir(), &["test", "--quiet"])
        .status()
        .context("failed to run cargo test in tests/axum-app")?;
    ensure!(status.success(), "the axum API fixture failed its tests");
    Ok(())
}

/// The generated code obeys the architecture it generates: the axum
/// fixture passes `rok sqlc check-arch` (ADR 0016) with no warning,
/// reading the fixture's `rok-data-sqlc.toml`.
#[test]
fn axum_app_passes_check_arch() -> Result<()> {
    let _guard = AXUM_APP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    generate_axum_app()?;

    let output = Command::new(env!("CARGO_BIN_EXE_rok"))
        .arg("sqlc")
        .arg("check-arch")
        .current_dir(axum_app_dir())
        .output()
        .context("failed to run rok sqlc check-arch")?;
    ensure!(
        output.status.success() && output.stdout.is_empty() && output.stderr.is_empty(),
        "the axum API fixture fails check-arch:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn axum_app_generated_code_is_rustfmt_clean() -> Result<()> {
    let _guard = AXUM_APP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let files = generate_axum_app()?;
    let files: Vec<&PathBuf> = files
        .iter()
        .filter(|file| file.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    ensure!(!files.is_empty(), "the axum API fixture generated no files");

    let output = Command::new("rustfmt")
        .args(["--edition", "2024", "--check"])
        .args(&files)
        .output()
        .context("failed to run rustfmt (install it with `rustup component add rustfmt`)")?;
    ensure!(
        output.status.success(),
        "the generated axum API code is not rustfmt-clean:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}
