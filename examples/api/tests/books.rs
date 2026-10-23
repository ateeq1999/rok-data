//! `/api/v1/books`: a book of an author, the books of an author, and the
//! lookup by ISBN. The foreign key and the unique constraint are the
//! failures.

mod support;

use axum::http::{Method, StatusCode};
use rok_api_example::db::authors::models::{Author, NewAuthor};
use rok_api_example::db::books::models::{Book, NewBook};
use serde_json::json;
use sqlx::PgPool;
use support::{Database, TestResult, call, user};

/// Inserts an author with `name` and `email`.
async fn author(pool: &PgPool, name: &str, email: &str) -> Result<uuid::Uuid, rok_data::Error> {
    let row = Author::create(
        pool,
        NewAuthor {
            id: None,
            name: name.to_owned(),
            email: email.to_owned(),
        },
    )
    .await?;
    Ok(row.id.into())
}

/// Inserts a book of `author_id`.
async fn book(
    pool: &PgPool,
    author_id: uuid::Uuid,
    title: &str,
    isbn: &str,
) -> Result<Book, rok_data::Error> {
    Book::create(
        pool,
        NewBook {
            id: None,
            author_id: author_id.into(),
            title: title.to_owned(),
            isbn: isbn.to_owned(),
            published_at: None,
        },
    )
    .await
}

#[tokio::test]
async fn a_caller_creates_a_book() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    let author_id = author(&db.pool, "Ada", "ada@example.com").await?;
    let body = json!({
        "author_id": author_id.to_string(),
        "title": "Notes on the engine",
        "isbn": "978-0-00-000000-1",
    });

    let answer = call(
        &db.pool,
        Some(&user()?),
        Method::POST,
        "/api/v1/books",
        Some(body),
    )
    .await?;
    assert_eq!(answer.status, StatusCode::CREATED, "{}", answer.body);
    assert_eq!(answer.body["title"], "Notes on the engine");
    assert_eq!(answer.body["isbn"], "978-0-00-000000-1");
    db.drop().await
}

#[tokio::test]
async fn a_book_of_an_unknown_author_is_a_conflict() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    let body = json!({
        "author_id": uuid::Uuid::now_v7().to_string(),
        "title": "Orphan",
        "isbn": "978-0-00-000000-2",
    });

    let answer = call(
        &db.pool,
        Some(&user()?),
        Method::POST,
        "/api/v1/books",
        Some(body),
    )
    .await?;
    assert_eq!(answer.status, StatusCode::CONFLICT, "{}", answer.body);
    assert_eq!(answer.code(), "conflict");
    db.drop().await
}

#[tokio::test]
async fn a_book_is_found_by_its_isbn() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    let author_id = author(&db.pool, "Bo", "bo@example.com").await?;
    let row = book(&db.pool, author_id, "The compiler", "978-0-00-000000-3").await?;

    let uri = format!("/api/v1/books/by-isbn/{}", row.isbn);
    let answer = call(&db.pool, Some(&user()?), Method::GET, &uri, None).await?;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.body["title"], "The compiler");
    db.drop().await
}

#[tokio::test]
async fn the_books_of_an_author_are_listed() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    let author_id = author(&db.pool, "Cleo", "cleo@example.com").await?;
    book(&db.pool, author_id, "First", "978-0-00-000000-4").await?;
    book(&db.pool, author_id, "Second", "978-0-00-000000-5").await?;

    let uri = format!("/api/v1/authors/{author_id}/books");
    let answer = call(&db.pool, Some(&user()?), Method::GET, &uri, None).await?;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let titles: Vec<&str> = answer
        .body
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|book| book["title"].as_str())
        .collect();
    assert_eq!(titles, ["First", "Second"]);
    db.drop().await
}
