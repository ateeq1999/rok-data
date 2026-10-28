//! `/api/v1/authors`: the list (with `q` and `sort`), one author, the
//! create and the update. Each route's happy path and one failure.

mod support;

use axum::http::{Method, StatusCode};
use rok_api_example::db::authors::models::{Author, NewAuthor};
use rok_data::Model;
use serde_json::json;
use sqlx::PgPool;
use support::{Database, TestResult, call, user};

/// Inserts an author with `name` and `email`.
async fn seed(pool: &PgPool, name: &str, email: &str) -> Result<Author, rok_data::Error> {
    Author::create(
        pool,
        NewAuthor {
            id: None,
            name: name.to_owned(),
            email: email.to_owned(),
        },
    )
    .await
}

#[tokio::test]
async fn a_caller_lists_authors_matching_q_and_sorted() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    seed(&db.pool, "Bob", "bob@example.com").await?;
    seed(&db.pool, "Alice", "alice@example.com").await?;
    seed(&db.pool, "Carol", "carol@example.com").await?;
    let token = user()?;

    let answer = call(
        &db.pool,
        Some(&token),
        Method::GET,
        "/api/v1/authors?q=example&sort=email&order=asc&per_page=2",
        None,
    )
    .await?;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.body["total"], 3);
    assert_eq!(answer.body["page"], 1);
    assert_eq!(answer.body["per_page"], 2);
    let emails: Vec<&str> = answer.body["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["email"].as_str())
        .collect();
    assert_eq!(emails, ["alice@example.com", "bob@example.com"]);

    let answer = call(
        &db.pool,
        Some(&token),
        Method::GET,
        "/api/v1/authors?q=carol",
        None,
    )
    .await?;
    assert_eq!(answer.body["total"], 1, "{}", answer.body);
    db.drop().await
}

#[tokio::test]
async fn listing_authors_needs_a_token() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    let answer = call(&db.pool, None, Method::GET, "/api/v1/authors", None).await?;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
    assert_eq!(answer.code(), "unauthorized");
    db.drop().await
}

#[tokio::test]
async fn a_caller_gets_an_author() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    let row = seed(&db.pool, "Dana", "dana@example.com").await?;

    let uri = format!("/api/v1/authors/{}", row.id);
    let answer = call(&db.pool, Some(&user()?), Method::GET, &uri, None).await?;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.body["name"], "Dana");
    assert_eq!(answer.body["email"], "dana@example.com");
    assert!(answer.body["created_at"].is_string(), "{}", answer.body);
    db.drop().await
}

#[tokio::test]
async fn getting_a_missing_author_is_not_found() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    let uri = format!("/api/v1/authors/{}", uuid::Uuid::now_v7());
    let answer = call(&db.pool, Some(&user()?), Method::GET, &uri, None).await?;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(answer.code(), "not_found");
    db.drop().await
}

#[tokio::test]
async fn a_caller_creates_an_author() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    let body = json!({"name": "Erin", "email": "erin@example.com"});

    let answer = call(
        &db.pool,
        Some(&user()?),
        Method::POST,
        "/api/v1/authors",
        Some(body),
    )
    .await?;
    assert_eq!(answer.status, StatusCode::CREATED, "{}", answer.body);
    assert_eq!(answer.body["name"], "Erin");
    assert_eq!(answer.body["email"], "erin@example.com");

    // The row is in the database, with the id the answer carries.
    let id: uuid::Uuid = match answer.body["id"].as_str().and_then(|id| id.parse().ok()) {
        Some(id) => id,
        None => return Err(format!("the answer carries no id: {}", answer.body).into()),
    };
    let stored = Author::find(&db.pool, id).await?;
    assert_eq!(stored.map(|author| author.name), Some("Erin".to_owned()));
    db.drop().await
}

#[tokio::test]
async fn a_second_author_with_the_same_email_fails_validation() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    seed(&db.pool, "Fay", "fay@example.com").await?;
    let body = json!({"name": "Fay again", "email": "fay@example.com"});

    let answer = call(
        &db.pool,
        Some(&user()?),
        Method::POST,
        "/api/v1/authors",
        Some(body),
    )
    .await?;
    // The `unique` pre-check names the field (engine ADR 0019).
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(answer.code(), "validation_failed");
    assert_eq!(
        answer.body["error"]["details"]["email"][0]["code"],
        "unique"
    );
    db.drop().await
}

#[tokio::test]
async fn a_caller_updates_an_author() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    let row = seed(&db.pool, "Gil", "gil@example.com").await?;

    let uri = format!("/api/v1/authors/{}", row.id);
    let body = json!({"name": "Gilbert"});
    let answer = call(&db.pool, Some(&user()?), Method::PATCH, &uri, Some(body)).await?;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.body["name"], "Gilbert");
    assert_eq!(answer.body["email"], "gil@example.com");

    let stored = Author::find(&db.pool, row.id).await?;
    assert_eq!(stored.map(|author| author.name), Some("Gilbert".to_owned()));
    db.drop().await
}
