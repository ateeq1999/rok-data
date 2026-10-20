//! The web framework the API features are generated for (ADR 0016):
//! actix-web (the default) or axum. The framework decides the routes, the
//! handlers and the shared tools; the bodies, the service, the listing and
//! the OpenAPI document are the same either way.
//!
//! Each framework has its own module, [`actix`] and [`axum`], holding the
//! emission that differs. [`Framework`] is the one place that chooses
//! between them, so no renderer asks which framework it writes for.

use super::feature::Feature;
use super::layout::Uses;
use crate::ApiConfig;
use crate::ApiFramework;

/// The framework the features are generated for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Framework {
    /// actix-web.
    ActixWeb,
    /// axum.
    Axum,
}

/// The status an answer carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    /// `200`.
    Ok,
    /// `201`.
    Created,
    /// `204`.
    NoContent,
}

/// Where a parameter goes in a handler's signature: actix-web's request
/// comes first, axum's last (it takes the body).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    /// The first parameter.
    First,
    /// The last one.
    Last,
}

/// The request a handler reads a caller from.
pub(crate) struct Request {
    /// The parameter, such as `req: HttpRequest`.
    pub param: String,
    /// Where it goes.
    pub place: Place,
}

impl Framework {
    /// The framework `api` asks for.
    pub(crate) fn of(api: &ApiConfig) -> Self {
        match api.framework {
            ApiFramework::ActixWeb => Framework::ActixWeb,
            ApiFramework::Axum => Framework::Axum,
        }
    }

    /// The feature's `routes.rs`: the URL of every endpoint, and the
    /// handler that answers it.
    pub(crate) fn routes(self, feature: &Feature<'_>) -> String {
        match self {
            Framework::ActixWeb => super::actix::routes(feature),
            Framework::Axum => super::axum::routes(feature),
        }
    }

    /// The `use` lines a handlers file needs.
    pub(crate) fn handler_uses(self, uses: &mut Uses) {
        match self {
            Framework::ActixWeb => super::actix::handler_uses(uses),
            Framework::Axum => super::axum::handler_uses(uses),
        }
    }

    /// The service parameter of a handler: the service it calls.
    pub(crate) fn service_param(self, uses: &mut Uses, service: &str) -> String {
        match self {
            Framework::ActixWeb => super::actix::service_param(uses, service),
            Framework::Axum => super::axum::service_param(uses, service),
        }
    }

    /// The path parameter of a handler: the row's key.
    pub(crate) fn path_param(self, uses: &mut Uses, ty: &str) -> String {
        match self {
            Framework::ActixWeb => super::actix::path_param(uses, ty),
            Framework::Axum => super::axum::path_param(uses, ty),
        }
    }

    /// The query parameter of a handler: a list's query.
    pub(crate) fn query_param(self, uses: &mut Uses, ty: &str) -> String {
        match self {
            Framework::ActixWeb => super::actix::query_param(uses, ty),
            Framework::Axum => super::axum::query_param(uses, ty),
        }
    }

    /// The body parameter of a handler: the body it validates.
    pub(crate) fn body_param(self, uses: &mut Uses, ty: &str) -> String {
        match self {
            Framework::ActixWeb => super::actix::body_param(uses, ty),
            Framework::Axum => super::axum::body_param(uses, ty),
        }
    }

    /// How the path's value is read.
    pub(crate) fn read_path(self, binding: &str) -> String {
        match self {
            Framework::ActixWeb => super::actix::read_path(binding),
            Framework::Axum => super::axum::read_path(binding),
        }
    }

    /// How the query's value is read.
    pub(crate) fn read_query(self, binding: &str) -> String {
        match self {
            Framework::ActixWeb => super::actix::read_query(binding),
            Framework::Axum => super::axum::read_query(binding),
        }
    }

    /// The handler's return type: the answer, or the error.
    pub(crate) fn result(self) -> &'static str {
        match self {
            Framework::ActixWeb => super::actix::result(),
            Framework::Axum => super::axum::result(),
        }
    }

    /// The answer's opening, the text the shared code replaces to return
    /// the answer early.
    pub(crate) fn answer_lead(self) -> &'static str {
        match self {
            Framework::ActixWeb => super::actix::answer_lead(),
            Framework::Axum => super::axum::answer_lead(),
        }
    }

    /// The path parameter of a relation's attach or detach: the row's key
    /// and the related row's.
    pub(crate) fn pair_path_param(self, uses: &mut Uses, key: &str, other: &str) -> String {
        match self {
            Framework::ActixWeb => super::actix::pair_path_param(uses, key, other),
            Framework::Axum => super::axum::pair_path_param(uses, key, other),
        }
    }

    /// The answer: a body, with the headers.
    pub(crate) fn answer(
        self,
        output: &mut String,
        uses: &mut Uses,
        status: Status,
        headers: &[(&str, &str)],
        body: &str,
    ) {
        match self {
            Framework::ActixWeb => super::actix::answer(output, uses, status, headers, body),
            Framework::Axum => super::axum::answer(output, uses, status, headers, body),
        }
    }

    /// The answer: no body.
    pub(crate) fn answer_empty(self, output: &mut String, uses: &mut Uses, status: Status) {
        match self {
            Framework::ActixWeb => super::actix::answer_empty(output, uses, status),
            Framework::Axum => super::axum::answer_empty(output, uses, status),
        }
    }

    /// The request a handler reads a caller from, when it reads one.
    pub(crate) fn request_param(self, uses: &mut Uses) -> Option<Request> {
        match self {
            Framework::ActixWeb => super::actix::request_param(uses),
            Framework::Axum => super::axum::request_param(uses),
        }
    }

    /// How a caller is read from the request.
    pub(crate) fn read_caller(self, uses: &mut Uses, extractor: &str) -> String {
        match self {
            Framework::ActixWeb => super::actix::read_caller(uses, extractor),
            Framework::Axum => super::axum::read_caller(uses, extractor),
        }
    }

    /// How a tenant is read from the request.
    pub(crate) fn read_tenant(self, uses: &mut Uses, ty: &str) -> String {
        match self {
            Framework::ActixWeb => super::actix::read_tenant(uses, ty),
            Framework::Axum => super::axum::read_tenant(uses, ty),
        }
    }

    /// The statement that opens the request's parts, for the frameworks
    /// whose callers are read from them.
    pub(crate) fn open_parts(self) -> Option<String> {
        match self {
            Framework::ActixWeb => super::actix::open_parts(),
            Framework::Axum => super::axum::open_parts(),
        }
    }

    /// The root's `generated.rs`: every feature, declared, and the wiring
    /// the application mounts.
    pub(crate) fn generated(self, features: &[Feature<'_>], api: &ApiConfig) -> String {
        match self {
            Framework::ActixWeb => super::actix::generated(features, api),
            Framework::Axum => super::axum::generated(features, api),
        }
    }

    /// What the root's `generated.rs` holds, in the README's table.
    pub(crate) fn root_summary(self) -> &'static str {
        match self {
            Framework::ActixWeb => "`register_services` and `configure_all`",
            Framework::Axum => "`router`",
        }
    }
}
