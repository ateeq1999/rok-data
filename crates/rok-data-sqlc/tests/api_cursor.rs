//! `[api] cursor`: a list paged by cursor through the library entry point —
//! what it generates, and the tables it refuses. The list's behaviour is
//! tested by running it in `tests/api-app` (its `labels`) and
//! `tests/axum-app` (its `notes`).

use std::fs;

use anyhow::Result;
use rok_data_sqlc::{ApiConfig, Config, Derive, Extractor};
use tempfile::TempDir;

const SCHEMA: &str = "CREATE TABLE posts (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    subtitle TEXT
);
CREATE TABLE events (kind TEXT NOT NULL);
";

/// The configuration `rok sqlc init` writes in `dir`, with `posts`'s list
/// paged by cursor.
fn config(dir: &TempDir) -> Result<Config> {
    fs::write(dir.path().join("schema.sql"), SCHEMA)?;
    let mut config = Config::default();
    config.schema = vec![dir.path().join("schema.sql")];
    config.output = dir.path().join("src/db");
    config.derive = vec![Derive::Serde, Derive::Validate];
    config.extractors = vec![Extractor::ActixWeb];
    let mut api = ApiConfig::new(dir.path().join("src/features"));
    api.cursor = vec!["posts".into()];
    api.search.insert("posts".into(), vec!["title".into()]);
    config.api = Some(api);
    Ok(config)
}

#[test]
fn a_cursor_list_takes_the_cursor_query_and_answers_the_cursor_page() -> Result<()> {
    let dir = TempDir::new()?;
    rok_data_sqlc::generate(&config(&dir)?)?;
    let read = |path: &str| fs::read_to_string(dir.path().join(path));

    let dto = read("src/features/posts/dto.rs")?;
    assert!(
        dto.contains("pub type ListPostsQuery = CursorListQuery<PostSort>;"),
        "{dto}"
    );
    let service = read("src/features/posts/service_api.rs")?;
    assert!(
        service.contains("-> Result<CursorResponse<Post>, AppError>"),
        "{service}"
    );
    assert!(service.contains("repo::list_by_cursor("), "{service}");
    let listing = read("src/db/posts/listing.rs")?;
    assert!(
        listing.contains("pub async fn list_by_cursor<'e, E>("),
        "{listing}"
    );
    assert!(!listing.contains("fn list_matching"), "{listing}");
    assert!(
        listing.contains("pub fn order(self, direction: Direction)"),
        "{listing}"
    );

    // The shared tools gain the cursor query and page; the data layer the
    // pattern `q` matches with.
    let pagination = read("src/shared/pagination.rs")?;
    assert!(
        pagination.contains("pub struct CursorListQuery<"),
        "{pagination}"
    );
    assert!(pagination.contains("pub fn cursor_error("), "{pagination}");
    let response = read("src/shared/response.rs")?;
    assert!(
        response.contains("pub struct CursorResponse<T>"),
        "{response}"
    );
    assert!(read("src/db/search.rs")?.contains("pub fn contains_pattern("));
    Ok(())
}

#[test]
fn without_a_cursor_list_the_shared_tools_have_no_cursor() -> Result<()> {
    let dir = TempDir::new()?;
    let mut config = config(&dir)?;
    config.api.as_mut().expect("the API").cursor.clear();
    rok_data_sqlc::generate(&config)?;
    for path in [
        "src/shared/pagination.rs",
        "src/shared/response.rs",
        "src/db/search.rs",
    ] {
        let text = fs::read_to_string(dir.path().join(path))?;
        assert!(!text.contains("Cursor"), "{path}: {text}");
        assert!(!text.contains("contains_pattern"), "{path}: {text}");
    }
    Ok(())
}

#[test]
fn a_cursor_list_needs_a_primary_key_and_sort_keys_that_are_never_null() -> Result<()> {
    let dir = TempDir::new()?;
    let mut config = config(&dir)?;
    let api = config.api.as_mut().expect("the API");
    api.cursor = vec!["events".into()];
    let error = format!("{:#}", rok_data_sqlc::generate(&config).unwrap_err());
    assert!(error.contains("`events` is in `api.cursor`"), "{error}");
    assert!(error.contains("primary key"), "{error}");

    let api = config.api.as_mut().expect("the API");
    api.cursor = vec!["posts".into()];
    api.sort.insert("posts".into(), vec!["subtitle".into()]);
    let error = format!("{:#}", rok_data_sqlc::generate(&config).unwrap_err());
    assert!(error.contains("sort key `subtitle` can be NULL"), "{error}");
    Ok(())
}
