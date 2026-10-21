//! axum's side of the API features (engine ADR 0016): the routes, the handlers'
//! syntax, and the root's wiring. actix-web's is [`super::actix`].

use super::feature::Feature;
use super::framework::{Place, Request, Status};
use super::layout::Uses;
use super::relation_routes;
use crate::ApiConfig;
use crate::generator::{FN_CALL_WIDTH, HEADER, MAX_WIDTH, push_doc};

/// Renders `routes.rs`: the router of every endpoint, under the feature's
/// URL, with the feature's service as an extension. Fixed paths come before
/// the key's, which would match them too.
pub fn routes(feature: &Feature<'_>) -> String {
    let key_path = feature.key_path();
    let mut routes: Vec<(String, &str, String)> = Vec::new();
    for endpoint in &feature.endpoints {
        let path = endpoint.path(&key_path);
        routes.push((path, endpoint.method(), endpoint.name().to_owned()));
    }
    for lookup in &feature.lookups {
        routes.push((lookup.path(), "get", lookup.function.clone()));
    }
    routes.extend(relation_routes::routes(feature));
    // Fewer `{...}` segments first, so `/search` and `/by-...` come before
    // `/{id}`; the order is otherwise kept.
    routes.sort_by_key(|(path, _, _)| path.matches('{').count());

    let mut uses = Uses::default();
    uses.add("axum::Router");
    uses.add("axum::extract::Extension");
    uses.add("std::sync::Arc");
    uses.add("super::handlers");
    for (_, method, _) in &routes {
        uses.add(&format!("axum::routing::{method}"));
    }
    uses.add(&format!("super::service::{}", feature.service()));

    let mut output = format!(
        "{HEADER}\n//! The `{}` routes, under `{}`.\n\n",
        feature.table.display_name(),
        feature.url
    );
    output.push_str(&uses.render());
    push_doc(
        &mut output,
        0,
        &[&format!(
            "Mounts every `{}` endpoint under `{}`.",
            feature.table.display_name(),
            feature.url
        )],
    );
    let service = feature.service();
    output.push_str(&format!(
        "pub fn routes(service: {service}) -> Router {{\n    Router::new()"
    ));
    for (path, method, handler) in &routes {
        let path = format!("{}{path}", feature.url);
        let to = format!("{method}(handlers::{handler})");
        let one_line = format!("        .route(\"{path}\", {to})");
        let args = format!("\"{path}\", {to}");
        if one_line.len() <= MAX_WIDTH && args.len() <= FN_CALL_WIDTH {
            output.push('\n');
            output.push_str(&one_line);
        } else {
            output.push_str(&format!(
                "\n        .route(\n            \"{path}\",\n            {to},\n        )"
            ));
        }
    }
    output.push_str("\n        .layer(Extension(Arc::new(service)))\n}\n");
    output
}

/// The `use` lines a handlers file needs.
pub fn handler_uses(uses: &mut Uses) {
    uses.add("axum::response::IntoResponse");
    uses.add("axum::response::Response");
}

/// The service parameter: `Extension(service): Extension<Arc<NotesService>>`.
pub fn service_param(uses: &mut Uses, service: &str) -> String {
    uses.add("axum::extract::Extension");
    uses.add("std::sync::Arc");
    format!("Extension(service): Extension<Arc<{service}>>")
}

/// The path parameter: `Path(path): Path<NoteId>`.
pub fn path_param(uses: &mut Uses, ty: &str) -> String {
    uses.add("crate::shared::validation::Path");
    format!("Path(path): Path<{ty}>")
}

/// The query parameter: `Query(query): Query<ListNotesQuery>`.
pub fn query_param(uses: &mut Uses, ty: &str) -> String {
    uses.add("crate::shared::validation::Query");
    format!("Query(query): Query<{ty}>")
}

/// The body parameter: `body: ValidatedJson<CreateNoteDto>`.
pub fn body_param(uses: &mut Uses, ty: &str) -> String {
    uses.add("crate::shared::validation::ValidatedJson");
    format!("body: ValidatedJson<{ty}>")
}

/// How the path's value is read: the path, which the pattern already
/// unwrapped.
pub fn read_path(binding: &str) -> String {
    binding.to_owned()
}

/// How the query's value is read: the query, which the pattern already
/// unwrapped.
pub fn read_query(binding: &str) -> String {
    binding.to_owned()
}

/// The handler's return type.
pub fn result() -> &'static str {
    "Result<Response, AppError>"
}

/// The answer's opening.
pub fn answer_lead() -> &'static str {
    "Ok("
}

/// The path parameter of a relation's attach or detach: the row's key and
/// the related row's, the tuple broken over lines as rustfmt breaks it when
/// the parameter is too long.
pub fn pair_path_param(uses: &mut Uses, key: &str, other: &str) -> String {
    uses.add("crate::shared::validation::Path");
    let pair = format!("{key}, {other}");
    if pair.len() <= crate::generator::FN_CALL_WIDTH {
        format!("Path(path): Path<({pair})>")
    } else {
        format!("Path(path): Path<(\n        {key},\n        {other},\n    )>")
    }
}

/// The answer: a body, with the headers.
pub fn answer(
    output: &mut String,
    uses: &mut Uses,
    status: Status,
    headers: &[(&str, &str)],
    body: &str,
) {
    uses.add("axum::Json");
    let status = match status {
        Status::Ok => "StatusCode::OK",
        Status::Created => "StatusCode::CREATED",
        Status::NoContent => "StatusCode::NO_CONTENT",
    };
    if headers.is_empty() {
        output.push_str(&format!("    Ok(Json({body}).into_response())\n"));
        return;
    }
    uses.add("axum::http::StatusCode");
    uses.add("axum::http::header");
    let headers: Vec<String> = headers
        .iter()
        .map(|(name, value)| format!("(header::{name}, {value})"))
        .collect();
    let headers = headers.join(", ");
    // The tuple's items fit one call's width: the answer stays on one
    // line, as rustfmt leaves it.
    let items = format!("{status}, [{headers}], Json({body})");
    if items.len() <= FN_CALL_WIDTH {
        output.push_str(&format!("    Ok(({items}).into_response())\n"));
        return;
    }
    // Otherwise the tuple is broken over lines, as rustfmt breaks it, and
    // the headers with it when they do not fit either.
    let headers = if headers.len() <= FN_CALL_WIDTH {
        format!("[{headers}]")
    } else {
        format!(
            "[\n            {},\n        ]",
            headers.replace(", ", ",\n            ")
        )
    };
    output.push_str(&format!(
        "    Ok((\n        {status},\n        {headers},\n        Json({body}),\n    )\n        .into_response())\n"
    ));
}

/// The answer: no body.
pub fn answer_empty(output: &mut String, uses: &mut Uses, status: Status) {
    uses.add("axum::http::StatusCode");
    let status = match status {
        Status::Ok => "StatusCode::OK",
        Status::Created => "StatusCode::CREATED",
        Status::NoContent => "StatusCode::NO_CONTENT",
    };
    output.push_str(&format!("    Ok({status}.into_response())\n"));
}

/// The request a handler reads a caller from: `req: Request`, last (it
/// takes the body).
pub fn request_param(uses: &mut Uses) -> Option<Request> {
    uses.add("axum::extract::Request");
    Some(Request {
        param: "req: Request".to_owned(),
        place: Place::Last,
    })
}

/// How a caller is read from the request's parts:
/// `AdminUser::from_request_parts(&mut parts, &()).await?`.
pub fn read_caller(uses: &mut Uses, extractor: &str) -> String {
    uses.add("axum::extract::FromRequestParts");
    format!("{extractor}::from_request_parts(&mut parts, &()).await?")
}

/// How a tenant is read from the request's parts:
/// `Tenant::<uuid::Uuid>::from_request_parts(&mut parts, &()).await?`.
pub fn read_tenant(uses: &mut Uses, ty: &str) -> String {
    uses.add("axum::extract::FromRequestParts");
    format!("Tenant::<{ty}>::from_request_parts(&mut parts, &()).await?")
}

/// The statement that opens the request's parts, where the callers are
/// read from.
pub fn open_parts() -> Option<String> {
    Some("    let (mut parts, _) = req.into_parts();\n".to_owned())
}

/// Renders `generated.rs`: every feature, declared, the router the
/// application mounts, and the `OpenAPI` document.
pub fn generated(features: &[Feature<'_>], api: &ApiConfig) -> String {
    let mut output = format!(
        "{HEADER}
//! Every generated API feature, declared from its own folder, and the
//! function `app.rs` calls: [`router`].

use axum::Router;
use sqlx::PgPool;
"
    );
    if api.openapi {
        output.push_str(super::api_doc::USES);
    }
    output.push_str(&super::root::modules(features));
    output.push_str(&super::root::related(features));
    output.push_str(&router(features));
    if api.openapi {
        output.push_str(&super::api_doc::render(features));
    }
    output
}

/// `router`: every feature's routes, each with its service over `pool`.
fn router(features: &[Feature<'_>]) -> String {
    let pool = if features.is_empty() { "_pool" } else { "pool" };
    let mut output = format!(
        "\n/// The whole router: every feature's routes, each with its service over\n/// `pool`.\npub fn router({pool}: &PgPool) -> Router {{\n"
    );
    // The services whose names make the call too long are built first.
    let mut services = String::new();
    let mut merges = String::new();
    for feature in features {
        let module = feature.module();
        let service = feature.service();
        let call = format!("{module}::routes({module}::{service}::new(pool.clone()))");
        if call.len() <= FN_CALL_WIDTH && format!("        .merge({call})").len() <= MAX_WIDTH {
            merges.push_str(&format!("        .merge({call})\n"));
            continue;
        }
        services.push_str(&format!(
            "    let {module}_service = {module}::{service}::new(pool.clone());\n"
        ));
        merges.push_str(&format!(
            "        .merge({module}::routes({module}_service))\n"
        ));
    }
    output.push_str(&services);
    output.push_str("    Router::new()\n");
    output.push_str(&merges);
    output.push_str("}\n");
    output
}
