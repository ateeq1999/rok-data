//! `/api/v1/loans`: lending a book and returning it. The `remove`
//! endpoint is skipped in `rok.toml` — a loan is returned, not deleted — so
//! the route is not mounted at all.

mod support;

use axum::http::{Method, StatusCode};
use rok_api_example::db::authors::models::NewAuthor;
use rok_api_example::db::authors::write as write_author;
use rok_api_example::db::books::models::NewBook;
use rok_api_example::db::books::write as write_book;
use rok_api_example::db::loans::models::NewLoan;
use rok_api_example::db::loans::write;
use serde_json::json;
use support::{Database, TestResult, call, user};

/// Inserts an author and a book of theirs, and answers the book's id.
async fn book(pool: &sqlx::PgPool) -> Result<uuid::Uuid, rok_data::Error> {
    let author = write_author::create(
        pool,
        NewAuthor {
            id: None,
            name: "Dara".to_owned(),
            email: "dara@example.com".to_owned(),
        },
    )
    .await?;
    let book = write_book::create(
        pool,
        NewBook {
            id: None,
            author_id: author.id,
            title: "On lending".to_owned(),
            isbn: "978-0-00-000000-6".to_owned(),
            published_at: None,
        },
    )
    .await?;
    Ok(book.id.into())
}

#[tokio::test]
async fn a_caller_lends_a_book() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    let book_id = book(&db.pool).await?;
    let body = json!({
        "book_id": book_id.to_string(),
        "borrower": "Hana",
    });
    let answer = call(
        &db.pool,
        Some(&user()?),
        Method::POST,
        "/api/v1/loans",
        Some(body),
    )
    .await?;
    assert_eq!(answer.status, StatusCode::CREATED, "{}", answer.body);
    assert_eq!(answer.body["borrower"], "Hana");
    assert!(answer.body["returned_at"].is_null(), "{}", answer.body);
    db.drop().await
}

#[tokio::test]
async fn a_loan_is_returned_by_an_update() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    let author = write_author::create(
        &db.pool,
        NewAuthor {
            id: None,
            name: "Ivo".to_owned(),
            email: "ivo@example.com".to_owned(),
        },
    )
    .await?;
    let book = write_book::create(
        &db.pool,
        NewBook {
            id: None,
            author_id: author.id,
            title: "On returning".to_owned(),
            isbn: "978-0-00-000000-7".to_owned(),
            published_at: None,
        },
    )
    .await?;
    let loan = write::create(
        &db.pool,
        NewLoan {
            id: None,
            book_id: book.id,
            borrower: "Jo".to_owned(),
            returned_at: None,
        },
    )
    .await?;
    let uri = format!("/api/v1/loans/{}", loan.id);
    let body = json!({"returned_at": "2026-10-20T12:00:00Z"});
    let answer = call(&db.pool, Some(&user()?), Method::PATCH, &uri, Some(body)).await?;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        answer.body["returned_at"].is_string(),
        "the loan is returned: {}",
        answer.body
    );
    db.drop().await
}

#[tokio::test]
async fn a_loan_is_not_deleted() -> TestResult {
    let Some(db) = Database::fresh().await? else {
        return Ok(());
    };
    let uri = format!("/api/v1/loans/{}", uuid::Uuid::now_v7());

    // The `remove` endpoint is skipped, so the route takes no `DELETE`.
    let answer = call(&db.pool, Some(&user()?), Method::DELETE, &uri, None).await?;
    assert_eq!(
        answer.status,
        StatusCode::METHOD_NOT_ALLOWED,
        "{}",
        answer.body
    );
    db.drop().await
}
