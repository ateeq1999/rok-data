//! `/api/v1/labels`: a list paged by cursor (`[api] cursor`): forward and
//! back through the pages, stable while rows are added, `q` and `sort`, and
//! the `400` of a cursor that is not one of the list's.

mod common;

use actix_web::http::StatusCode;
use common::{Answer, Database, assert_error, create, get};
use serde_json::{Value, json};

const LABELS: &str = "/api/v1/labels";

/// The labels named `names`, created in that order.
async fn labels(db: &Database, names: &[&str]) {
    for name in names {
        create(db, LABELS, json!({"name": name})).await;
    }
}

/// The names on a page.
fn names(answer: &Answer) -> Vec<&str> {
    answer.body["items"]
        .as_array()
        .unwrap_or_else(|| panic!("a page: {answer:?}"))
        .iter()
        .map(|item| item["name"].as_str().expect("a name"))
        .collect()
}

/// A page's `next` or `prev` cursor.
fn cursor<'a>(answer: &'a Answer, which: &str) -> &'a str {
    answer.body[which].as_str().expect("a cursor")
}

#[actix_web::test]
async fn the_pages_go_forward_and_back_and_stay_put_while_rows_are_added() {
    let Some(db) = Database::fresh().await else {
        return;
    };
    labels(&db, &["a", "b", "c", "d", "e"]).await;
    let by_name = format!("{LABELS}?sort=name&order=asc&per_page=2");

    let first = get(&db, &by_name).await;
    assert_eq!(first.status, StatusCode::OK, "{first:?}");
    assert_eq!(names(&first), ["a", "b"]);
    assert_eq!(first.body["prev"], Value::Null);
    assert_eq!(first.body["per_page"], 2);

    let second = get(&db, &format!("{by_name}&cursor={}", cursor(&first, "next"))).await;
    assert_eq!(names(&second), ["c", "d"]);

    // A row sorted before the cursor shifts no page after it.
    labels(&db, &["aa"]).await;
    let third = get(&db, &format!("{by_name}&cursor={}", cursor(&second, "next"))).await;
    assert_eq!(names(&third), ["e"]);
    assert_eq!(third.body["next"], Value::Null);

    // Back: the page before `c` now holds the new row, and has one before
    // it in turn.
    let back = get(&db, &format!("{by_name}&cursor={}", cursor(&third, "prev"))).await;
    assert_eq!(names(&back), ["c", "d"]);
    let start = get(&db, &format!("{by_name}&cursor={}", cursor(&back, "prev"))).await;
    assert_eq!(names(&start), ["aa", "b"]);
    assert!(start.body["prev"].is_string(), "{start:?}");
    db.drop().await;
}

#[actix_web::test]
async fn the_default_sort_and_q_apply() {
    let Some(db) = Database::fresh().await else {
        return;
    };
    labels(&db, &["red", "green", "ready", "blue"]).await;

    // `id`, newest first, unless `sort` and `order` say otherwise.
    let page = get(&db, &format!("{LABELS}?per_page=3")).await;
    assert_eq!(names(&page), ["blue", "ready", "green"]);
    let rest = get(&db, &format!("{LABELS}?per_page=3&cursor={}", cursor(&page, "next"))).await;
    assert_eq!(names(&rest), ["red"]);

    // `q` matches the name, ignoring case.
    let matched = get(&db, &format!("{LABELS}?q=RE&sort=name&order=asc")).await;
    assert_eq!(names(&matched), ["green", "ready", "red"]);
    db.drop().await;
}

#[actix_web::test]
async fn a_cursor_that_is_not_the_lists_answers_400() {
    let Some(db) = Database::fresh().await else {
        return;
    };
    labels(&db, &["a", "b"]).await;

    let answer = get(&db, &format!("{LABELS}?cursor=not-a-cursor")).await;
    assert_error(&answer, StatusCode::BAD_REQUEST, "bad_request");

    // A cursor of the `id` order doesn't fit the `name` order.
    let by_id = get(&db, &format!("{LABELS}?per_page=1")).await;
    let next = cursor(&by_id, "next");
    let answer = get(&db, &format!("{LABELS}?sort=name&cursor={next}")).await;
    assert_error(&answer, StatusCode::BAD_REQUEST, "bad_request");
    db.drop().await;
}
