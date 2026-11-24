//! `rok sqlc check-arch` on how an actix-web project's features mount their
//! routes: a feature's `web::scope` covering another feature's route, and a
//! fixed path mounted after a parameterised one that matches it. Both are
//! warnings: printed on stderr, counted on the passing line, never failing
//! the run.

use super::check_arch::{check_arch, project, stdout};
use super::*;

#[test]
fn a_covering_scope_and_a_fixed_path_behind_a_parameter_are_warnings() -> Result<()> {
    let dir = project(&[
        ("src/features/partner/mod.rs", "pub mod routes;\n"),
        (
            "src/features/partner/routes.rs",
            "use actix_web::web;\n\
             pub fn routes(cfg: &mut web::ServiceConfig) {\n\
             \x20   cfg.service(web::scope(\"/partner\").route(\"/{id}\", web::get().to(get_one)));\n\
             }\n",
        ),
        ("src/features/offers/mod.rs", "pub mod routes;\n"),
        (
            "src/features/offers/routes.rs",
            "use actix_web::web;\n\
             pub fn routes(cfg: &mut web::ServiceConfig) {\n\
             \x20   cfg.service(web::resource(\"/partner/{id}/offers\").route(web::get().to(list)));\n\
             \x20   cfg.service(web::resource(\"/offers/{id}\").route(web::get().to(get_one)));\n\
             \x20   cfg.service(web::resource(\"/offers/search\").route(web::post().to(search)));\n\
             }\n",
        ),
    ])?;
    let output = check_arch(&dir, &[]);
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "checked 2 features: no violations, 2 warnings\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr).replace("\r\n", "\n");
    assert_eq!(
        stderr.lines().skip(1).collect::<Vec<_>>(),
        [
            "warning: src/features/offers/routes.rs:5: route-order: `/offers/search` is \
             mounted after `/offers/{id}` (line 4), which matches it and answers 405 for a \
             method it doesn't route: mount the fixed path first",
            "warning: src/features/partner/routes.rs:3: covering-scope: \
             `web::scope(\"/partner\")` covers feature `offers`'s `/partner/{id}/offers`, \
             which then answers 404 when this scope is mounted first: mount \
             `web::resource`s, not a scope",
        ],
        "{stderr}"
    );
    Ok(())
}
