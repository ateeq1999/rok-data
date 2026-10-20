//! The axum backend of the API generator (ADR 0016): the `[api] framework`
//! key chooses it, and the routes, the handlers, the shared tools and the
//! root's wiring are axum's. The actix backend's output does not change:
//! it is the default, and the fixture's checked-in files are its proof.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use rok_data_sqlc::{ApiConfig, ApiFramework, Config, Derive, Extractor};
use tempfile::TempDir;

const SCHEMA: &str = "CREATE TABLE users (id UUID PRIMARY KEY, email TEXT NOT NULL);
CREATE TABLE posts (id BIGSERIAL PRIMARY KEY, title TEXT NOT NULL);
";

struct Project {
    dir: TempDir,
}

impl Project {
    fn new() -> Result<Self> {
        let project = Project {
            dir: TempDir::new()?,
        };
        fs::write(project.path("schema.sql"), SCHEMA)?;
        Ok(project)
    }

    fn path(&self, path: &str) -> PathBuf {
        self.dir.path().join(path)
    }

    fn read(&self, path: &str) -> Result<String> {
        Ok(fs::read_to_string(self.path(path))?)
    }

    /// The configuration of an axum project: the axum extractor and the
    /// axum backend.
    fn config(&self) -> Config {
        let mut config = Config::default();
        config.schema = vec![self.path("schema.sql")];
        config.output = self.path("src/db");
        config.derive = vec![Derive::Serde, Derive::Validate];
        config.extractors = vec![Extractor::Axum];
        let mut api = ApiConfig::new(self.path("src/features"));
        api.framework = ApiFramework::Axum;
        config.api = Some(api);
        config
    }
}

/// The axum routes are a `Router`'s, the handlers take axum's extractors
/// and answer with axum's response, and the root wires them into a
/// `Router`.
#[test]
fn the_axum_backend_generates_axums_code() -> Result<()> {
    let project = Project::new()?;
    let config = project.config();
    rok_data_sqlc::generate(&config)?;

    let routes = project.read("src/features/users/routes.rs")?;
    for expected in [
        "use axum::Router;",
        "use axum::routing::{delete, get, patch, post};",
        "pub fn routes(service: UsersService) -> Router {",
        "        .route(\"/api/v1/users\", get(handlers::list))",
        "        .route(\"/api/v1/users\", post(handlers::create))",
        "        .layer(Extension(Arc::new(service)))",
    ] {
        assert!(routes.contains(expected), "{expected}\n{routes}");
    }
    assert!(!routes.contains("actix_web"), "{routes}");

    let handlers = project.read("src/features/users/handlers.rs")?;
    for expected in [
        "use axum::Json;",
        "use axum::response::{IntoResponse, Response};",
        "    Extension(service): Extension<Arc<UsersService>>,",
        "    Path(path): Path<UserId>,",
        "    body: ValidatedJson<CreateUserDto>,",
        ") -> Result<Response, AppError> {",
        "    Ok(Json(response).into_response())",
    ] {
        assert!(handlers.contains(expected), "{expected}\n{handlers}");
    }
    assert!(!handlers.contains("actix_web"), "{handlers}");

    let generated = project.read("src/features/generated.rs")?;
    for expected in [
        "use axum::Router;",
        "pub fn router(pool: &PgPool) -> Router {",
        "        .merge(users::routes(users::UsersService::new(pool.clone())))",
    ] {
        assert!(generated.contains(expected), "{expected}\n{generated}");
    }
    assert!(!generated.contains("register_services"), "{generated}");

    // The shared tools are axum's too.
    let health = project.read("src/shared/health.rs")?;
    assert!(health.contains("use axum::Json;"), "{health}");
    assert!(!health.contains("actix_web"), "{health}");
    let tenant = project.read("src/shared/tenant.rs")?;
    assert!(
        tenant.contains("use axum::extract::FromRequestParts;"),
        "{tenant}"
    );
    let validation = project.read("src/shared/validation.rs")?;
    assert!(
        validation.contains("pub use crate::db::http::axum::ValidatedJson;"),
        "{validation}"
    );
    Ok(())
}

/// The actix backend is the default: without the key, the routes are a
/// scope's and the handlers answer with a `HttpResponse`.
#[test]
fn the_actix_backend_is_the_default() -> Result<()> {
    let project = Project::new()?;
    let mut config = project.config();
    config.extractors = vec![Extractor::ActixWeb];
    let mut api = config.api.take().expect("the API is configured");
    api.framework = ApiFramework::ActixWeb;
    config.api = Some(api);
    rok_data_sqlc::generate(&config)?;

    let routes = project.read("src/features/users/routes.rs")?;
    for expected in [
        "use actix_web::web;",
        "pub fn routes(cfg: &mut web::ServiceConfig) {",
        "        web::scope(\"/api/v1/users\")",
        "            .route(\"\", web::get().to(handlers::list))",
    ] {
        assert!(routes.contains(expected), "{expected}\n{routes}");
    }
    let handlers = project.read("src/features/users/handlers.rs")?;
    assert!(
        handlers.contains("    service: web::Data<UsersService>,"),
        "{handlers}"
    );
    assert!(
        handlers.contains("    Ok(HttpResponse::Ok().json(response))"),
        "{handlers}"
    );
    Ok(())
}

/// The framework the configuration names is the one the features are
/// generated for: the axum routes and handlers together, never one of each.
#[test]
fn the_framework_chooses_the_whole_feature() -> Result<()> {
    let project = Project::new()?;
    let mut config = project.config();
    config.extractors = vec![Extractor::ActixWeb, Extractor::Axum];
    rok_data_sqlc::generate(&config)?;

    let routes = project.read("src/features/users/routes.rs")?;
    assert!(routes.contains("axum::Router"), "{routes}");
    let handlers = project.read("src/features/users/handlers.rs")?;
    assert!(
        handlers.contains("use axum::response::{IntoResponse, Response};"),
        "{handlers}"
    );
    assert!(!handlers.contains("actix_web"), "{handlers}");
    Ok(())
}
