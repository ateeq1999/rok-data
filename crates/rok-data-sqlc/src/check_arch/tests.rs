//! `check-arch` on projects of its own, on real files: rule 1's axum side,
//! and the actix wording the default must keep byte for byte. The CLI's
//! shape of these checks (`rok sqlc check-arch` reading the config file) is
//! `rok-cli`'s suite's (`tests/cli/check_arch.rs`), and the `AGENTS.md`
//! sections the rules render are `init`'s suite's.

use std::fs;

use anyhow::Result;
use tempfile::TempDir;

use super::{Project, check};
use crate::ApiFramework;

/// Checks a scratch project: `files` (paths under `src/`) written to disk
/// in an empty directory, checked for `framework`, one line per violation.
fn violations(framework: ApiFramework, files: &[(&str, &str)]) -> Result<Vec<String>> {
    let dir = TempDir::new()?;
    let root = dir.path().to_path_buf();
    for (path, text) in files {
        let path = root.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, text)?;
    }
    let project = Project {
        root,
        features: dir.path().join("src/features"),
        db_path: "crate::db".to_owned(),
        framework,
    };
    Ok(check(&project)?
        .violations
        .iter()
        .map(|v| v.to_string())
        .collect())
}

/// The message `check-arch` prints under `axum`, when the code at `path`
/// names the framework outside the files rule 1 allows. Naming it once
/// keeps every axum assertion here in step with the renderer.
fn axum_message(target: &str, path: &str) -> String {
    format!(
        "{path}: rule-1: `{target}`: only routes.rs, handlers.rs, app.rs, main.rs, \
         error.rs, features/generated.rs, shared/ and db/ may use axum"
    )
}

/// One axum project's files the generation and the scaffolds write, each in
/// its own file: the rule every one of them lives under holds at once.
const AXUM_GENERATED: &[(&str, &str)] = &[
    // The wiring at the root, the errors and the entry: `app.rs` and
    // `error.rs` may name `axum`; `main.rs` starts through the library.
    (
        "src/app.rs",
        "use axum::Router;\nuse axum::middleware::Next;\npub fn f(_: Router, _: Next) {}\n",
    ),
    (
        "src/error.rs",
        "use axum::http::StatusCode;\nuse axum::response::{IntoResponse, Response};\npub enum AppError { X }\n",
    ),
    ("src/main.rs", "fn main() {}\n"),
    // The generated wiring and the data layer's extractor module: the
    // features' `generated.rs`, and `db/http/axum.rs` under the data layer.
    // The generated root's `sqlx` import rides rule 2's exemption, too.
    (
        "src/features/generated.rs",
        "use axum::Router;\nuse sqlx::PgPool;\npub fn router(_: Router, _: &PgPool) {}\n",
    ),
    (
        "src/db/http/axum.rs",
        "use axum::extract::FromRequest;\nuse axum::response::IntoResponse;\npub struct V;\n",
    ),
    // The shared tools: the auth module an axum project keeps.
    (
        "src/shared/auth.rs",
        "use axum::extract::FromRequestParts;\nuse axum::http::request::Parts;\n",
    ),
];

#[test]
fn an_axum_project_passes_what_the_axum_generation_writes() -> Result<()> {
    let mut files: Vec<(&str, &str)> = AXUM_GENERATED.to_vec();
    files.extend_from_slice(&[
        // The feature's HTTP files: exactly the two the rule allows.
        (
            "src/features/notes/routes.rs",
            "use axum::routing::get;\nuse axum::Router;\npub fn routes() -> Router { Router::new() }\n",
        ),
        (
            "src/features/notes/handlers.rs",
            "use axum::response::{IntoResponse, Response};\npub async fn list() -> Response { ().into_response() }\n",
        ),
        // And the service, which names none of the framework.
        ("src/features/notes/service.rs", "pub struct NotesService;\n"),
    ]);
    assert_eq!(
        violations(ApiFramework::Axum, &files)?,
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn an_axum_project_flags_the_framework_outside_its_files() -> Result<()> {
    let mut files: Vec<(&str, &str)> = AXUM_GENERATED.to_vec();
    files.extend_from_slice(&[
        (
            "src/features/notes/routes.rs",
            "use axum::Router;\npub fn routes() -> Router { Router::new() }\n",
        ),
        // The stray: a feature's service file names the framework.
        (
            "src/features/notes/service.rs",
            "use axum::http::StatusCode;\npub struct NotesService { pub s: String }\n",
        ),
    ]);
    let expected = [axum_message(
        "axum::http::StatusCode",
        "src/features/notes/service.rs:1",
    )];
    assert_eq!(
        violations(ApiFramework::Axum, &files)?.as_slice(),
        &expected
    );
    Ok(())
}

#[test]
fn the_axum_rule_names_which_file_may_import() -> Result<()> {
    // A feature file that is no HTTP file (`dto.rs`), and a root file that
    // is no wiring (`config.rs`): the rule's file sets are real, and the
    // message lists the files that may.
    let files = [(
        "src/features/notes/dto.rs",
        "use axum::http::StatusCode;\npub struct D;\n",
    )];
    let expected = [axum_message(
        "axum::http::StatusCode",
        "src/features/notes/dto.rs:1",
    )];
    assert_eq!(
        violations(ApiFramework::Axum, &files)?.as_slice(),
        &expected
    );
    let files = [("src/config.rs", "use axum::Router;\npub struct C;\n")];
    let expected = [axum_message("axum::Router", "src/config.rs:1")];
    assert_eq!(
        violations(ApiFramework::Axum, &files)?.as_slice(),
        &expected
    );
    Ok(())
}

/// The actix rule's message, byte for byte: the default framework's rule
/// prints what it printed before the rule followed the config.
#[test]
fn the_actix_rule_is_what_it_always_printed() -> Result<()> {
    let files = [(
        "src/features/notes/service.rs",
        "use actix_web::HttpResponse;\npub struct NotesService;\n",
    )];
    assert_eq!(
        violations(ApiFramework::ActixWeb, &files)?,
        [
            "src/features/notes/service.rs:1: rule-1: `actix_web::HttpResponse`: only routes.rs, handlers.rs, app.rs, main.rs, error.rs, features/generated.rs, shared/ and db/ may use actix_web"
        ],
    );
    Ok(())
}

/// `src/app/` is `app.rs` split up: its files may name the framework and
/// `sqlx` as `app.rs` does, and it is no unknown top-level directory.
/// Another directory still is.
#[test]
fn the_wiring_may_be_split_into_app() -> Result<()> {
    for framework in [ApiFramework::ActixWeb, ApiFramework::Axum] {
        let watched = match framework {
            ApiFramework::ActixWeb => "actix_web",
            ApiFramework::Axum | ApiFramework::Grpc => "axum",
        };
        let router = format!("use {watched}::http::StatusCode;\nuse sqlx::PgPool;\n");
        let files = [
            ("src/app.rs", "mod router;\nmod state;\n"),
            ("src/app/router.rs", router.as_str()),
            (
                "src/app/state.rs",
                "use sqlx::PgPool;\npub struct S(PgPool);\n",
            ),
        ];
        assert_eq!(violations(framework, &files)?, Vec::<String>::new());
    }
    let files = [("src/wiring/router.rs", "pub struct R;\n")];
    let found = violations(ApiFramework::Axum, &files)?;
    assert!(
        found
            .iter()
            .any(|v| v.contains("unknown top-level directory `wiring`")),
        "{found:?}"
    );
    Ok(())
}

#[test]
fn rule_1_reports_the_other_framework_but_in_the_data_layer() -> Result<()> {
    // The other framework's crate is a rule-1 violation everywhere but the
    // data layer, whose `extractors` may serve both (engine ADR 0021, A7).
    let other = |framework: &str, target: &str, crate_name: &str| {
        format!(
            "src/features/notes/service.rs:1: rule-1: `{target}`: the project uses \
             {framework} (`[api] framework`), so nothing but db/ may use {crate_name}"
        )
    };
    let files = [
        (
            "src/features/notes/service.rs",
            "use axum::Router;
pub struct NotesService;
",
        ),
        (
            "src/db/http.rs",
            "pub mod axum { use axum::Json; }
",
        ),
    ];
    assert_eq!(
        violations(ApiFramework::ActixWeb, &files)?,
        [other("actix_web", "axum::Router", "axum")]
    );
    let files = [
        (
            "src/features/notes/service.rs",
            "use actix_web::HttpResponse;
pub struct NotesService;
",
        ),
        (
            "src/db/http.rs",
            "pub mod actix { use actix_web::FromRequest; }
",
        ),
    ];
    assert_eq!(
        violations(ApiFramework::Axum, &files)?,
        [other("axum", "actix_web::HttpResponse", "actix_web")]
    );
    Ok(())
}

/// `check-arch` reads the framework the config file sets, through the same
/// discovery and resolve the generation does; without the key, the default
/// stays the default.
#[test]
fn the_config_sets_the_watched_framework() -> Result<()> {
    use crate::config_file;

    let dir = TempDir::new()?;
    let config = dir.path().join(config_file::FILE_NAME);
    fs::write(
        &config,
        "[api]\noutput = \"src/features\"\nframework = \"axum\"\n",
    )?;
    let api = config_file::load(&config)?
        .api
        .resolve()
        .ok_or_else(|| anyhow::anyhow!("the config resolved to no API"))?;
    assert_eq!(api.framework, ApiFramework::Axum);

    // Through that resolve, rule 1 polices `axum` in the project the
    // config describes.
    fs::create_dir_all(dir.path().join("src/features/notes"))?;
    fs::write(
        dir.path().join("src/features/notes/service.rs"),
        "use axum::http::StatusCode;\n",
    )?;
    let project = Project {
        root: dir.path().to_path_buf(),
        features: api.output,
        db_path: api.db_path,
        framework: api.framework,
    };
    let violations: Vec<String> = check(&project)?
        .violations
        .iter()
        .map(|v| v.to_string())
        .collect();
    assert_eq!(
        violations,
        [axum_message(
            "axum::http::StatusCode",
            "src/features/notes/service.rs:1"
        )]
    );

    // And without the key, the API names actix_web, the default.
    fs::write(&config, "[api]\noutput = \"src/features\"\n")?;
    let api = config_file::load(&config)?
        .api
        .resolve()
        .ok_or_else(|| anyhow::anyhow!("the config resolved to no API"))?;
    assert_eq!(api.framework, ApiFramework::ActixWeb);
    Ok(())
}

/// A service controls its transactions: `sqlx::Transaction`, a
/// savepoint's `begin` (through `Acquire`) and the runtime's
/// `rok_data::transaction` pass rule 2; writing SQL still fails it.
#[test]
fn a_service_may_control_a_transaction_but_not_write_sql() -> Result<()> {
    let service = "\
use sqlx::{Acquire, PgPool, Postgres, Transaction};
pub struct NotesService(PgPool);
impl NotesService {
    pub async fn audited(&self, tx: &mut Transaction<'_, Postgres>) -> Result<(), AppError> {
        let savepoint = tx.begin().await?;
        Ok(savepoint.commit().await?)
    }
    pub async fn whole(&self) -> Result<(), rok_data::Error> {
        rok_data::transaction(&self.0, |_tx| Box::pin(async move { Ok(()) })).await
    }
}
";
    let files = [("src/features/notes/service.rs", service)];
    assert_eq!(
        violations(ApiFramework::Axum, &files)?,
        Vec::<String>::new()
    );

    let writes =
        "pub async fn f(pool: &sqlx::PgPool) { let _ = sqlx::query(\"DELETE FROM notes\"); }\n";
    let files = [("src/features/notes/service.rs", writes)];
    let found = violations(ApiFramework::Axum, &files)?;
    assert!(
        found.iter().any(|v| v.contains("rule-2: `sqlx::query`")),
        "{found:?}"
    );
    Ok(())
}

/// Rule 4's three messages say what a feature may do as well as what it
/// may not: its queries join and read any table, and only writing another
/// feature's table goes through that feature's service. Read as forbidding
/// reads, the old wording sent a project to a second call for a column a
/// join would have read.
#[test]
fn rule_4_says_a_features_queries_may_read_any_table() -> Result<()> {
    let files = [
        ("src/features/mod.rs", "pub mod labels;\npub mod notes;\n"),
        ("src/features/labels/mod.rs", "pub mod service;\n"),
        (
            "src/features/labels/service.rs",
            "pub struct LabelsService;\n",
        ),
        ("src/features/notes/mod.rs", "pub mod service;\n"),
        (
            "src/features/notes/service.rs",
            "use crate::features::labels::service::LabelsService;\n\
             pub fn f() { crate::db::labels::write::create(); }\n\
             pub fn g() { crate::db::queries::labels::count(); }\n",
        ),
    ];
    let said = "a feature's queries may join and read any table; writing another \
                feature's table goes through that feature's service:";
    assert_eq!(
        violations(ApiFramework::ActixWeb, &files)?,
        [
            format!(
                "src/features/notes/service.rs:1: rule-4: \
                 `crate::features::labels::service::LabelsService`: feature `notes` refers \
                 to another feature; {said} declare a port in its service.rs; a pure helper \
                 with no feature state belongs in shared/"
            ),
            format!(
                "src/features/notes/service.rs:2: cross-feature-write: \
                 `crate::db::labels::write::create`: feature `notes` writes another table; \
                 {said} declare a port in its service.rs and implement it in app.rs"
            ),
            format!(
                "src/features/notes/service.rs:3: cross-feature-query: \
                 `crate::db::queries::labels::count`: feature `notes` calls another \
                 feature's query module; {said} declare a port in its service.rs and \
                 implement it in app.rs"
            ),
        ],
    );
    Ok(())
}
