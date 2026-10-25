//! serde and axum integration.
#![cfg(all(feature = "serde", feature = "axum"))]

use axum_core::response::IntoResponse;
use http_body_util::BodyExt;
use rok_data::{Cursor, CursorPage, Error, Page, ValidationErrors};
use serde_json::{Value as Json, json};

#[test]
fn pages_and_cursors_serialize() {
    let page = Page::new(vec!["a", "b"], 5, 1, 2);
    assert_eq!(
        serde_json::to_value(&page).unwrap(),
        json!({"items": ["a", "b"], "total": 5, "page": 1, "per_page": 2, "total_pages": 3})
    );

    let cursor = Cursor::new([rok_data::Value::from(7_i64)]);
    let back = Cursor::before([rok_data::Value::from(3_i64)]);
    let page = CursorPage {
        items: vec![1, 2],
        next: Some(cursor.clone()),
        prev: Some(back.clone()),
        per_page: 2,
    };
    let value = serde_json::to_value(&page).unwrap();
    assert_eq!(value["next"], json!(cursor.to_string()));
    assert_eq!(value["prev"], json!(back.to_string()));
    let parsed: Cursor = serde_json::from_value(value["next"].clone()).unwrap();
    assert_eq!(parsed, cursor);
    let parsed: Cursor = serde_json::from_value(value["prev"].clone()).unwrap();
    assert!(parsed.is_backward(), "the direction survives the token");
    assert!(serde_json::from_value::<Cursor>(json!("garbage!")).is_err());
    let last = CursorPage::<i32> {
        items: vec![],
        next: None,
        prev: None,
        per_page: 10,
    };
    assert_eq!(
        serde_json::to_value(&last).unwrap(),
        json!({"items": [], "next": null, "prev": null})
    );
}

async fn respond(err: Error) -> (u16, Json) {
    let response = err.into_response();
    let status = response.status().as_u16();
    assert_eq!(response.headers()["content-type"], "application/json");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn errors_become_json_responses() {
    let (status, body) = respond(Error::NotFound {
        table: "users",
        key: Some("7".into()),
    })
    .await;
    assert_eq!((status, body["error"].as_str()), (404, Some("not_found")));
    assert!(body["message"].as_str().unwrap().contains("users"));

    let mut errors = ValidationErrors::new();
    errors.add("email", "email", "must be a valid email address");
    let (status, body) = respond(Error::Validation(errors)).await;
    assert_eq!(status, 422);
    assert_eq!(
        body["fields"],
        json!([{"field": "email", "code": "email", "message": "must be a valid email address"}])
    );

    let (status, body) = respond(Error::Conflict {
        table: "users",
        key: "1".into(),
    })
    .await;
    assert_eq!((status, body["error"].as_str()), (409, Some("conflict")));
    let (status, _) = respond(Error::InvalidCursor("bad".into())).await;
    assert_eq!(status, 400);
    let (status, body) = respond(Error::hook("not allowed")).await;
    assert_eq!(
        (status, body["message"].as_str()),
        (422, Some("not allowed"))
    );

    // Internal details are not leaked.
    let (status, body) = respond(Error::Config("secret connection string".into())).await;
    assert_eq!(status, 500);
    assert_eq!(
        body,
        json!({"error": "internal_error", "message": "internal server error"})
    );
}

#[tokio::test]
async fn database_errors_map_to_statuses() {
    let Some(test_db) = rok_data::testing::TestDb::create().await.unwrap() else {
        return;
    };
    let db = test_db.db();
    db.execute("CREATE TABLE parents (id INT PRIMARY KEY); CREATE TABLE kids (id INT PRIMARY KEY, parent INT REFERENCES parents)")
        .await
        .unwrap();
    rok_data::raw("INSERT INTO parents VALUES (1)")
        .execute(db)
        .await
        .unwrap();
    let dup = rok_data::raw("INSERT INTO parents VALUES (1)")
        .execute(db)
        .await
        .unwrap_err();
    let (status, body) = respond(dup).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (409, Some("already_exists"))
    );
    assert!(
        !body["message"].as_str().unwrap().contains("parents_pkey"),
        "constraint names stay private"
    );
    let orphan = rok_data::raw("INSERT INTO kids VALUES (1, 42)")
        .execute(db)
        .await
        .unwrap_err();
    assert_eq!(respond(orphan).await.0, 422);
}

#[test]
fn links_keep_the_other_query_parameters() {
    use rok_data::wire::Links;
    let links = Links::from_request("/api/v1/books?sort=title&page=2&order=asc");
    assert_eq!(links.path(), "/api/v1/books");
    assert_eq!(links.page(3), "/api/v1/books?sort=title&order=asc&page=3");
    assert_eq!(
        links.cursor("l3432"),
        "/api/v1/books?sort=title&order=asc&cursor=l3432"
    );
    let named = Links::from_request("/books?p=2&q=rust")
        .page_param("p")
        .fragment("results");
    assert_eq!(named.page(1), "/books?q=rust&p=1#results");
    assert_eq!(Links::from_request("/books").page(2), "/books?page=2");
}

#[test]
fn a_page_answers_as_a_paginator_and_a_resource() {
    use rok_data::wire::Links;
    // Page 2 of 5 rows at 2 a page.
    let page = Page::new(vec!["c", "d"], 5, 2, 2);
    let links = || Links::from_request("/books?sort=title&page=2");
    assert_eq!(
        serde_json::to_value(page.paginator(links())).unwrap(),
        json!({
            "current_page": 2,
            "data": ["c", "d"],
            "first_page_url": "/books?sort=title&page=1",
            "from": 3,
            "last_page": 3,
            "last_page_url": "/books?sort=title&page=3",
            "next_page_url": "/books?sort=title&page=3",
            "path": "/books",
            "per_page": 2,
            "prev_page_url": "/books?sort=title&page=1",
            "to": 4,
            "total": 5
        })
    );
    assert_eq!(
        serde_json::to_value(page.resource(links())).unwrap(),
        json!({
            "data": ["c", "d"],
            "links": {
                "first": "/books?sort=title&page=1",
                "last": "/books?sort=title&page=3",
                "prev": "/books?sort=title&page=1",
                "next": "/books?sort=title&page=3"
            },
            "meta": {
                "current_page": 2, "from": 3, "last_page": 3, "path": "/books",
                "per_page": 2, "to": 4, "total": 5
            }
        })
    );
    // An empty page past the end: no positions, no next.
    let past = Page::new(Vec::<&str>::new(), 5, 9, 2);
    let value = serde_json::to_value(past.paginator(links())).unwrap();
    assert_eq!(
        (value["from"].clone(), value["to"].clone()),
        (json!(null), json!(null))
    );
    assert_eq!(value["next_page_url"], json!(null));
}

#[test]
fn simple_and_cursor_pages_answer_as_a_paginator_and_a_resource() {
    use rok_data::SimplePage;
    use rok_data::wire::Links;
    let simple = SimplePage::new(vec![1, 2], 1, 2, true);
    assert_eq!(
        serde_json::to_value(simple.paginator(Links::from_request("/n"))).unwrap(),
        json!({
            "current_page": 1, "data": [1, 2], "first_page_url": "/n?page=1",
            "from": 1, "next_page_url": "/n?page=2", "path": "/n", "per_page": 2,
            "prev_page_url": null, "to": 2
        })
    );
    let next = Cursor::new([rok_data::Value::from(9_i64)]);
    let prev = Cursor::before([rok_data::Value::from(3_i64)]);
    let cursor = CursorPage {
        items: vec![4, 5],
        next: Some(next.clone()),
        prev: Some(prev.clone()),
        per_page: 2,
    };
    let links = || Links::from_request("/n?cursor=old&q=x");
    assert_eq!(
        serde_json::to_value(cursor.paginator(links())).unwrap(),
        json!({
            "data": [4, 5], "path": "/n", "per_page": 2,
            "next_cursor": next.to_string(),
            "next_page_url": format!("/n?q=x&cursor={next}"),
            "prev_cursor": prev.to_string(),
            "prev_page_url": format!("/n?q=x&cursor={prev}")
        })
    );
    let resource = serde_json::to_value(cursor.resource(links())).unwrap();
    assert_eq!(resource["links"]["first"], json!(null));
    assert_eq!(resource["meta"]["next_cursor"], json!(next.to_string()));
}
