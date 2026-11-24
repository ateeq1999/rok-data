//! How the generated features mount: one resource per path, never a scope,
//! so a feature mounted under another's prefix answers in either mount
//! order; and fixed paths before the parameterised ones that match them, so
//! every method of each answers.

mod common;

use actix_web::http::{Method, StatusCode};
use actix_web::{App, HttpResponse, test as actix_test, web};
use common::{Database, call, create};
use rok_data_sqlc_api_app::app::configure;
use rok_data_sqlc_api_app::shared::auth::JwtKeys;
use serde_json::{Value, json};

const NOTES: &str = "/api/v1/notes";

/// A hand-written feature under the generated `notes`' prefix:
/// `GET /api/v1/notes/{id}/offers`.
fn offers(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/api/v1/notes/{id}/offers").route(web::get().to(list_offers)));
}

async fn list_offers(path: web::Path<String>) -> HttpResponse {
    HttpResponse::Ok().json(json!({"note": path.into_inner(), "offers": []}))
}

/// Where the hand-written feature mounts: before the generated ones or
/// after them.
#[derive(Debug, Clone, Copy)]
enum Order {
    HandWrittenFirst,
    GeneratedFirst,
}

/// Sends `method uri` to the application with the hand-written feature
/// mounted in `order`; returns the status and the JSON body (`Null` when
/// empty).
async fn send(db: &Database, order: Order, method: Method, uri: &str) -> (StatusCode, Value) {
    let pool = db.pool.clone();
    let keys = web::Data::new(JwtKeys::new("unused"));
    let app = App::new().configure(move |cfg| match order {
        Order::HandWrittenFirst => {
            offers(cfg);
            configure(cfg, &pool, &keys);
        }
        Order::GeneratedFirst => {
            configure(cfg, &pool, &keys);
            offers(cfg);
        }
    });
    let app = actix_test::init_service(app).await;
    let request = actix_test::TestRequest::default()
        .method(method)
        .uri(uri)
        .to_request();
    let response = actix_test::call_service(&app, request).await;
    let status = response.status();
    let bytes = actix_test::read_body(response).await;
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

#[actix_web::test]
async fn two_features_under_one_prefix_both_answer_in_either_mount_order() {
    let Some(db) = Database::fresh().await else {
        return;
    };
    let note = create(&db, NOTES, json!({"slug": "a", "title": "First"})).await;
    let id = note["id"].as_str().expect("an id");
    for order in [Order::HandWrittenFirst, Order::GeneratedFirst] {
        let (status, body) = send(&db, order, Method::GET, NOTES).await;
        assert_eq!(status, StatusCode::OK, "{order:?}: {body}");
        let (status, body) = send(&db, order, Method::GET, &format!("{NOTES}/{id}")).await;
        assert_eq!(status, StatusCode::OK, "{order:?}: {body}");
        assert_eq!(body, note);
        let uri = format!("{NOTES}/{id}/offers");
        let (status, body) = send(&db, order, Method::GET, &uri).await;
        assert_eq!(status, StatusCode::OK, "{order:?}: {body}");
        assert_eq!(body, json!({"note": id, "offers": []}));
    }
    db.drop().await;
}

/// `notes` has fixed paths (`/search`, `/by-slug/{value}`) beside the
/// key's (`/{id}`), which would match `/search` too: each answers each of
/// its methods.
#[actix_web::test]
async fn a_fixed_path_and_a_parameterised_one_both_answer_each_method() {
    let Some(db) = Database::fresh().await else {
        return;
    };
    let note = create(&db, NOTES, json!({"slug": "a", "title": "First"})).await;
    let id = note["id"].as_str().expect("an id");
    let one = format!("{NOTES}/{id}");

    let search = call(
        &db,
        Method::POST,
        &format!("{NOTES}/search"),
        Some(json!({})),
    )
    .await;
    assert_eq!(search.status, StatusCode::OK, "{search:?}");
    assert_eq!(search.body["items"], json!([note]));
    let found = call(&db, Method::GET, &format!("{NOTES}/by-slug/a"), None).await;
    assert_eq!(found.status, StatusCode::OK, "{found:?}");

    let read = call(&db, Method::GET, &one, None).await;
    assert_eq!(read.status, StatusCode::OK, "{read:?}");
    let patch = Some(json!({"title": "Second"}));
    let patched = call(&db, Method::PATCH, &one, patch).await;
    assert_eq!(patched.status, StatusCode::OK, "{patched:?}");
    assert_eq!(patched.body["title"], "Second");
    let removed = call(&db, Method::DELETE, &one, None).await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{removed:?}");

    // A method the path doesn't serve is a `405`, not another route's
    // answer.
    let put = call(&db, Method::PUT, &format!("{NOTES}/search"), None).await;
    assert_eq!(put.status, StatusCode::METHOD_NOT_ALLOWED, "{put:?}");
    db.drop().await;
}
