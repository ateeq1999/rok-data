//! actix-web's side of the API features (ADR 0016): the routes, the
//! handlers' syntax, and the root's wiring. axum's is [`super::axum`].

use super::feature::Feature;
use super::framework::{Place, Request, Status};
use super::layout::Uses;
use super::relation_routes;
use crate::ApiConfig;
use crate::generator::{FN_CALL_WIDTH, HEADER, MAX_WIDTH, push_doc};

/// Renders `routes.rs`: one scope under the feature's URL, which answers a
/// malformed path or query with the wire standard's `400`. Fixed paths
/// come before the key's, which would match them too.
pub fn routes(feature: &Feature<'_>) -> String {
    let mut output = format!(
        "{HEADER}\n//! The `{}` routes, under `{}`.\n\nuse actix_web::web;\n\nuse super::handlers;\nuse crate::shared::validation::{{path_config, query_config}};\n\n",
        feature.table.display_name(),
        feature.url
    );
    push_doc(
        &mut output,
        0,
        &[&format!(
            "Mounts every `{}` endpoint under `{}`.",
            feature.table.display_name(),
            feature.url
        )],
    );
    output.push_str("pub fn routes(cfg: &mut web::ServiceConfig) {\n    cfg.service(\n");
    output.push_str(&format!(
        "        web::scope({})\n            .app_data(path_config())\n            .app_data(query_config())",
        crate::generator::sql::str_literal(&feature.url)
    ));
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
    for (path, method, handler) in &routes {
        let to = format!("web::{method}().to(handlers::{handler})");
        let one_line = format!("            .route(\"{path}\", {to})");
        let args = format!("\"{path}\", {to}");
        if one_line.len() <= MAX_WIDTH && args.len() <= FN_CALL_WIDTH {
            output.push('\n');
            output.push_str(&one_line);
        } else {
            output.push_str(&format!(
                "\n            .route(\n                \"{path}\",\n                {to},\n            )"
            ));
        }
    }
    output.push_str(",\n    );\n}\n");
    output
}

/// The `use` lines a handlers file needs.
pub fn handler_uses(uses: &mut Uses) {
    uses.add("actix_web::HttpResponse");
    uses.add("actix_web::web");
}

/// The service parameter: `service: web::Data<NotesService>`.
pub fn service_param(uses: &mut Uses, service: &str) -> String {
    uses.add("actix_web::web");
    format!("service: web::Data<{service}>")
}

/// The path parameter: `path: web::Path<NoteId>`.
pub fn path_param(uses: &mut Uses, ty: &str) -> String {
    uses.add("actix_web::web");
    format!("path: web::Path<{ty}>")
}

/// The query parameter: `query: web::Query<ListNotesQuery>`.
pub fn query_param(uses: &mut Uses, ty: &str) -> String {
    uses.add("actix_web::web");
    format!("query: web::Query<{ty}>")
}

/// The body parameter: `body: ValidatedJson<CreateNoteDto>`.
pub fn body_param(uses: &mut Uses, ty: &str) -> String {
    uses.add("crate::shared::validation::ValidatedJson");
    format!("body: ValidatedJson<{ty}>")
}

/// How the path's value is read: `path.into_inner()`.
pub fn read_path(binding: &str) -> String {
    format!("{binding}.into_inner()")
}

/// How the query's value is read: `query.into_inner()`.
pub fn read_query(binding: &str) -> String {
    format!("{binding}.into_inner()")
}

/// The handler's return type.
pub fn result() -> &'static str {
    "Result<HttpResponse, AppError>"
}

/// The answer's opening.
pub fn answer_lead() -> &'static str {
    "Ok(HttpResponse"
}

/// The path parameter of a relation's attach or detach: the row's key and
/// the related row's, the tuple broken over lines as rustfmt breaks it when
/// the parameter is too long.
pub fn pair_path_param(_uses: &mut Uses, key: &str, other: &str) -> String {
    let pair = format!("{key}, {other}");
    if pair.len() <= crate::generator::FN_CALL_WIDTH {
        format!("path: web::Path<({pair})>")
    } else {
        format!("path: web::Path<(\n        {key},\n        {other},\n    )>")
    }
}

/// The answer: a body, with the headers, one header per line.
pub fn answer(
    output: &mut String,
    uses: &mut Uses,
    status: Status,
    headers: &[(&str, &str)],
    body: &str,
) {
    let status = match status {
        Status::Ok => "Ok",
        Status::Created => "Created",
        Status::NoContent => "NoContent",
    };
    if headers.is_empty() {
        output.push_str(&format!("    Ok(HttpResponse::{status}().json({body}))\n"));
        return;
    }
    uses.add("actix_web::http::header");
    output.push_str(&format!("    Ok(HttpResponse::{status}()\n"));
    for (name, value) in headers {
        output.push_str(&format!(
            "        .insert_header((header::{name}, {value}))\n"
        ));
    }
    output.push_str(&format!("        .json({body}))\n"));
}

/// The answer: no body.
pub fn answer_empty(output: &mut String, _uses: &mut Uses, status: Status) {
    let status = match status {
        Status::Ok => "Ok",
        Status::Created => "Created",
        Status::NoContent => "NoContent",
    };
    output.push_str(&format!("    Ok(HttpResponse::{status}().finish())\n"));
}

/// The request a handler reads a caller from: `req: HttpRequest`, first.
pub fn request_param(uses: &mut Uses) -> Option<Request> {
    uses.add("actix_web::HttpRequest");
    Some(Request {
        param: "req: HttpRequest".to_owned(),
        place: Place::First,
    })
}

/// How a caller is read from the request: `AdminUser::extract(&req).await?`.
pub fn read_caller(uses: &mut Uses, extractor: &str) -> String {
    uses.add("actix_web::FromRequest");
    format!("{extractor}::extract(&req).await?")
}

/// How a tenant is read from the request:
/// `Tenant::<uuid::Uuid>::extract(&req).await?`.
pub fn read_tenant(uses: &mut Uses, ty: &str) -> String {
    uses.add("actix_web::FromRequest");
    format!("Tenant::<{ty}>::extract(&req).await?")
}

/// actix-web's callers are read from the request itself: nothing to open.
pub fn open_parts() -> Option<String> {
    None
}

/// Renders `generated.rs`: every feature, declared, the services
/// registered, the routes mounted, and the `OpenAPI` document.
pub fn generated(features: &[Feature<'_>], api: &ApiConfig) -> String {
    let mut output = format!(
        "{HEADER}
//! Every generated API feature, declared from its own folder, and the two
//! functions `app.rs` calls: [`register_services`] and [`configure_all`].

use actix_web::web;
use sqlx::PgPool;
"
    );
    if api.openapi {
        output.push_str(super::api_doc::USES);
    }
    output.push_str(&super::root::modules(features));
    output.push_str(&super::root::related(features));
    output.push_str(&register_services(features));
    output.push_str(&configure_all(features));
    if api.openapi {
        output.push_str(&super::api_doc::render(features));
    }
    output
}

/// `register_services`: every feature's service as app data, each over
/// `pool`.
fn register_services(features: &[Feature<'_>]) -> String {
    let (cfg, pool) = if features.is_empty() {
        ("_cfg", "_pool")
    } else {
        ("cfg", "pool")
    };
    let mut output = format!(
        "\n/// Registers every feature's service as app data, each over `pool`.\npub fn register_services({cfg}: &mut web::ServiceConfig, {pool}: &PgPool) {{\n"
    );
    for feature in features {
        let service = format!("{}::{}", feature.module(), feature.service());
        let data = format!("web::Data::new({service}::new(pool.clone()))");
        let line = format!("    cfg.app_data({data});");
        if data.len() <= FN_CALL_WIDTH && line.len() <= MAX_WIDTH {
            output.push_str(&line);
            output.push('\n');
        } else {
            // A long name: built first, so each statement wraps simply.
            let call = format!("{service}::new(pool.clone());");
            if format!("    let service = {call}").len() <= MAX_WIDTH {
                output.push_str(&format!("    let service = {call}\n"));
            } else if format!("        {call}").len() <= MAX_WIDTH {
                output.push_str(&format!("    let service =\n        {call}\n"));
            } else {
                output.push_str(&format!(
                    "    let service =\n        {service}::new(\n            pool.clone(),\n        );\n"
                ));
            }
            output.push_str("    cfg.app_data(web::Data::new(service));\n");
        }
    }
    output.push_str("}\n");
    output
}

/// `configure_all`: every feature's routes, mounted.
fn configure_all(features: &[Feature<'_>]) -> String {
    let cfg = if features.is_empty() { "_cfg" } else { "cfg" };
    let mut output = format!(
        "\n/// Mounts every feature's routes.\npub fn configure_all({cfg}: &mut web::ServiceConfig) {{\n"
    );
    for feature in features {
        output.push_str(&format!("    {}::routes(cfg);\n", feature.module()));
    }
    output.push_str("}\n");
    output
}
