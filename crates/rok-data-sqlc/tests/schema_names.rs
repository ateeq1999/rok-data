//! Two `ToSchema` types with one component name: utoipa keeps one of them
//! in the document, so a hand-written DTO named like a generated one would
//! silently replace it and the Clients come out wrong. A run (and a check
//! run) fails naming both files instead; `#[schema(as = …)]` resolves it.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use rok_data_sqlc::{ApiConfig, Config, Derive, Extractor};
use tempfile::TempDir;

const SCHEMA: &str = "CREATE TABLE notes (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL
);
";

struct Project {
    dir: TempDir,
}

impl Project {
    fn generated() -> Result<Self> {
        let project = Project {
            dir: TempDir::new()?,
        };
        fs::write(project.path("schema.sql"), SCHEMA)?;
        rok_data_sqlc::generate(&project.config())?;
        Ok(project)
    }

    fn path(&self, path: &str) -> PathBuf {
        self.dir.path().join(path)
    }

    fn config(&self) -> Config {
        let mut config = Config::default();
        config.schema = vec![self.path("schema.sql")];
        config.output = self.path("src/db");
        config.derive = vec![Derive::Serde, Derive::Validate];
        config.extractors = vec![Extractor::ActixWeb];
        config.api = Some(ApiConfig::new(self.path("src/features")));
        config
    }

    /// The error a run (or a check run, with `check`) fails with.
    fn failure(&self, check: bool) -> String {
        let mut config = self.config();
        config.check = check;
        match rok_data_sqlc::generate(&config) {
            Ok(_) => panic!("the run should fail"),
            Err(error) => format!("{:#}", anyhow::Error::from(error)),
        }
    }
}

/// A hand-written response named like the generated `NoteResponse`.
const CLASHING: &str = "#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct NoteResponse {
    pub id: i64,
}
";

#[test]
fn a_hand_written_dto_named_like_a_generated_one_fails_the_run() -> Result<()> {
    let project = Project::generated()?;
    fs::write(project.path("src/features/notes/summary.rs"), CLASHING)?;

    for check in [false, true] {
        let message = project.failure(check);
        assert!(
            message.contains("two schemas are named `NoteResponse`"),
            "{message}"
        );
        assert!(message.contains("features/notes/dto.rs"), "{message}");
        assert!(message.contains("features/notes/summary.rs"), "{message}");
        assert!(message.contains("#[schema(as = "), "{message}");
    }
    Ok(())
}

#[test]
fn a_schema_as_rename_resolves_the_clash() -> Result<()> {
    let project = Project::generated()?;
    let renamed = CLASHING.replace(
        "pub struct",
        "#[schema(as = notes::NoteSummary)]\npub struct",
    );
    fs::write(project.path("src/features/notes/summary.rs"), renamed)?;

    rok_data_sqlc::generate(&project.config())?;
    Ok(())
}

#[test]
fn a_schema_as_rename_onto_a_taken_name_fails_the_run() -> Result<()> {
    let project = Project::generated()?;
    let renamed = "#[derive(serde::Serialize, utoipa::ToSchema)]
#[schema(as = NoteResponse)]
pub struct Summary {
    pub id: i64,
}
";
    fs::write(project.path("src/features/notes/summary.rs"), renamed)?;

    let message = project.failure(false);
    assert!(
        message.contains("two schemas are named `NoteResponse`"),
        "{message}"
    );
    Ok(())
}
