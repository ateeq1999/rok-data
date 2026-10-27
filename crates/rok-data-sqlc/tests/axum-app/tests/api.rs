//! The axum application under test: the router the fixture mounts, the
//! requests it answers, and the `OpenAPI` document it serves.
//!
//! The tests need no database: the document, the health route and the
//! answers that stop before the data layer are the same over a pool that
//! never connects.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

use rok_data_sqlc_axum_app::app::app;
use rok_data_sqlc_axum_app::shared::auth::{Claims, JwtKeys, Role};

/// The JWT secret the test application verifies tokens with.
const SECRET: &str = "test secret";

/// The application, over a pool that never connects.
fn application() -> Router {
    let pool = PgPool::connect_lazy("postgres://localhost/unused").expect("a well-formed URL");
    app(&pool, &JwtKeys::new(SECRET))
}

/// Sends `request` to the application and answers with the status and the
/// body.
async fn send(request: Request<Body>) -> (StatusCode, String) {
    let response = application()
        .oneshot(request)
        .await
        .expect("the application answers");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("a body")
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// A request to `uri`, with the method and the body.
fn request(method: &str, uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .expect("a request")
}

/// A bearer token for the caller `id` with `role`, signed with the secret
/// the test application verifies with, valid for an hour.
fn token(id: uuid::Uuid, role: Role) -> String {
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock after 1970")
        .as_secs()
        + 3600;
    let claims = Claims { sub: id, role, exp };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(SECRET.as_bytes()),
    )
    .expect("an encodable token")
}

/// The `error` object of an answer in the wire standard's shape.
fn error(body: &str) -> Value {
    let body: Value = serde_json::from_str(body).expect("a JSON body");
    body["error"].clone()
}

/// The served `OpenAPI` document.
async fn document() -> Value {
    let (status, body) = send(request("GET", "/api-docs/openapi.json", "")).await;
    assert_eq!(status, StatusCode::OK);
    serde_json::from_str(&body).expect("a JSON document")
}

/// The document lists every generated path: the feature's own endpoints,
/// its relationship routes, and the health ones.
#[tokio::test]
async fn the_document_lists_every_generated_path() {
    let document = document().await;
    let paths = document["paths"].as_object().expect("the paths");
    for path in [
        "/api/v1/notes",
        "/api/v1/notes/search",
        "/api/v1/notes/{id}",
        "/api/v1/notes/{id}/restore",
        "/api/v1/notes/{id}/labels",
        "/api/v1/notes/{id}/labels/{other}",
        "/api/v1/labels",
        "/api/v1/labels/{id}",
        "/health/live",
        "/health/ready",
    ] {
        assert!(paths.contains_key(path), "{path} is missing: {paths:?}");
    }
    // The list documents its query, the search its body. The notes' list
    // pages by cursor (`[api] cursor`): `cursor`, never `page`.
    let list = &paths["/api/v1/notes"]["get"];
    let names: Vec<&str> = list["parameters"]
        .as_array()
        .expect("the parameters")
        .iter()
        .filter_map(|parameter| parameter["name"].as_str())
        .collect();
    assert_eq!(names, ["cursor", "per_page", "q", "sort", "order"]);
}

/// The health route answers while the server answers at all.
#[tokio::test]
async fn the_health_route_answers() {
    let (status, body) = send(request("GET", "/health/live", "")).await;
    assert_eq!(status, StatusCode::OK);
    let body: Value = serde_json::from_str(&body).expect("a JSON body");
    assert_eq!(body["status"], "ok");
}

/// A route needing a caller answers `401` without a token, in the wire
/// standard's shape.
#[tokio::test]
async fn a_route_needing_a_caller_answers_401() {
    let (status, body) = send(request("GET", "/api/v1/notes", "")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(error(&body)["code"], "unauthorized");
    assert_eq!(error(&body)["message"], "a bearer token is required");
}

/// A path that doesn't parse answers `400`, in the wire standard's shape:
/// the path's extractor turns axum's own rejection into it.
#[tokio::test]
async fn a_malformed_path_answers_400() {
    let (status, body) = send(request("GET", "/api/v1/labels/not-a-number", "")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error(&body)["code"], "bad_request");
    // Worded as actix-web words it, so both frameworks answer the same.
    assert_eq!(
        error(&body)["message"],
        "Path deserialize error: can not parse \"not-a-number\" to a i64"
    );
}

/// A query string that doesn't parse answers `400`, worded as actix-web
/// words it, without the field path axum adds.
#[tokio::test]
async fn a_malformed_query_answers_400() {
    let (status, body) = send(request("GET", "/api/v1/labels?page=abc", "")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error(&body)["code"], "bad_request");
    assert_eq!(
        error(&body)["message"],
        "Query deserialize error: invalid digit found in string"
    );
}

/// A body that is not JSON answers `400`, in the wire standard's shape: the
/// validated-body extractor turns axum's own rejection into it.
#[tokio::test]
async fn a_malformed_body_answers_400() {
    let (status, body) = send(request("POST", "/api/v1/labels", "{")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error(&body)["code"], "bad_request");
}

/// A body that does not fit the type answers `422`, in the wire standard's
/// shape: the validated-body extractor turns axum's own rejection into it.
#[tokio::test]
async fn a_body_that_does_not_fit_answers_422() {
    let (status, body) = send(request("POST", "/api/v1/labels", r#"{"name": 42}"#)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error(&body)["code"], "validation_failed");
}

/// A caller with a token gets past the access check: the request then
/// fails on the database, which the test does not have.
#[tokio::test]
async fn a_caller_with_a_token_gets_past_the_access_check() {
    let token = token(uuid::Uuid::now_v7(), Role::User);
    let request = Request::builder()
        .uri("/api/v1/notes")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("a request");
    let (status, body) = send(request).await;
    // The tenant is missing, which is a wiring bug, not the caller's
    // fault: a `500`, never a `401`.
    assert_ne!(status, StatusCode::UNAUTHORIZED, "{body}");
}

/// `[api] pagination = "paginator"`: a list page answers the paginator
/// object, field for field what `rok_data::wire` writes for the same page,
/// its links keeping the request's other query parameters.
#[test]
fn a_list_page_answers_as_the_paginator_object() {
    use rok_data_sqlc_axum_app::shared::pagination::Pagination;
    use rok_data_sqlc_axum_app::shared::response::{PaginatedResponse, PaginatorResponse};
    use serde_json::json;

    // Page 2 of 5 rows at 2 a page.
    let page = PaginatedResponse::new(vec!["c", "d"], Pagination::new(Some(2), Some(2)), 5);
    let response = PaginatorResponse::new(page, "/books?sort=title&page=2");
    assert_eq!(
        serde_json::to_value(response).unwrap(),
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

    // A page past the end: no rows, no positions, no next page.
    let list = PaginatedResponse::new(Vec::<u8>::new(), Pagination::new(Some(9), Some(2)), 5);
    let past = serde_json::to_value(PaginatorResponse::new(list, "/books")).unwrap();
    assert_eq!(
        (past["from"].clone(), past["next_page_url"].clone()),
        (json!(null), json!(null))
    );
}

/// `[api] cursor`: a cursor page answers the paginator object, field for
/// field what `rok_data::wire` writes for a `CursorPage`, its links
/// replacing only the request's `cursor`.
#[test]
fn a_cursor_page_answers_as_the_paginator_object() {
    use rok_data::{Cursor, CursorPage, Value};
    use rok_data_sqlc_axum_app::shared::response::{CursorPaginatorResponse, CursorResponse};
    use serde_json::json;

    let page = CursorPage {
        items: vec!["c", "d"],
        next: Some(Cursor::new([Value::from(42_i64)])),
        prev: Some(Cursor::before([Value::from(41_i64)])),
        per_page: 2,
    };
    let response = CursorPaginatorResponse::new(
        CursorResponse::from(page),
        "/notes?sort=title&cursor=l3431&per_page=2",
    );
    assert_eq!(
        serde_json::to_value(response).unwrap(),
        json!({
            "data": ["c", "d"],
            "path": "/notes",
            "per_page": 2,
            "next_cursor": "l3432",
            "next_page_url": "/notes?sort=title&per_page=2&cursor=l3432",
            "prev_cursor": "-l3431",
            "prev_page_url": "/notes?sort=title&per_page=2&cursor=-l3431"
        })
    );

    // The first and only page: no cursor either way.
    let only = CursorPage {
        items: vec!["a"],
        next: None,
        prev: None,
        per_page: 20,
    };
    let only = CursorPaginatorResponse::new(CursorResponse::from(only), "/notes");
    let only = serde_json::to_value(only).unwrap();
    assert_eq!(
        (only["next_page_url"].clone(), only["prev_cursor"].clone()),
        (json!(null), json!(null))
    );
}

/// The list answers the cursor page in the paginator shape; the search,
/// which pages by number, the numbered one.
#[tokio::test]
async fn the_document_describes_each_page() {
    let document = document().await;
    for (path, method, shape) in [
        ("/api/v1/notes", "get", "CursorPaginatorResponse"),
        ("/api/v1/notes/search", "post", "PaginatorResponse"),
    ] {
        let answer = &document["paths"][path][method]["responses"]["200"];
        let schema = answer["content"]["application/json"]["schema"]["$ref"]
            .as_str()
            .unwrap_or_default();
        let name = schema.rsplit('/').next().unwrap_or_default();
        assert!(name.starts_with(&format!("{shape}_")), "{path}: {answer}");
    }
}
