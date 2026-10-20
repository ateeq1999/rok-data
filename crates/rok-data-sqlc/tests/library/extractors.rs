//! The derives and the HTTP extractors the generated code needs.

use super::*;

/// Without `derive`, nothing generated mentions `serde`.
#[test]
fn generated_code_mentions_serde_only_when_derived() -> Result<()> {
    let inputs = Inputs::new()?;
    rok_data_sqlc::generate(&inputs.config("plain"))?;
    for (path, text) in tree(&inputs.path("plain"))? {
        assert!(!text.contains("serde"), "{} mentions serde", path.display());
    }

    let mut config = inputs.config("serde");
    config.derive = vec![rok_data_sqlc::Derive::Serde];
    rok_data_sqlc::generate(&config)?;
    let models = fs::read_to_string(inputs.path("serde/users/models.rs"))?;
    let serde = "serde::Serialize,";
    // Each item's attributes (between its doc comment and the item) derive
    // serde; a typed id serializes as its inner value.
    let attributes = |item: &str| {
        let before = models.split(&format!("pub struct {item}")).next().unwrap();
        let doc_end = before.rfind("///").unwrap();
        before[doc_end..].to_owned()
    };
    assert!(attributes("UserId(").contains(serde), "{models}");
    assert!(
        attributes("UserId(").contains("#[serde(transparent)]\n"),
        "{models}"
    );
    for item in ["User ", "NewUser ", "UpdateUser "] {
        assert!(attributes(item).contains(serde), "{item}:\n{models}");
    }
    let hints = fs::read_to_string(inputs.path("serde/mod.rs"))?;
    assert!(
        hints.contains("//   serde = { version = \"1\", features = [\"derive\"] }\n"),
        "{hints}"
    );
    assert!(
        hints.contains("//   uuid = { version = \"1\", features = [\"serde\"] }\n"),
        "{hints}"
    );
    Ok(())
}

/// `Derive::Validate` needs `Derive::Serde`, and `extractors` need
/// `Derive::Validate`; a run missing either fails before writing anything.
#[test]
fn validate_needs_serde_and_extractors_need_validate() -> Result<()> {
    use rok_data_sqlc::{Derive, Extractor};
    let inputs = Inputs::new()?;

    let mut config = inputs.config("gen");
    config.derive = vec![Derive::Validate];
    let error = rok_data_sqlc::generate(&config).unwrap_err().to_string();
    assert!(
        error.contains("`validate` in `derive` needs `serde`"),
        "{error}"
    );

    config.derive = vec![Derive::Serde];
    config.extractors = vec![Extractor::Axum, Extractor::ActixWeb];
    let error = rok_data_sqlc::generate(&config).unwrap_err().to_string();
    assert!(
        error.contains("`extractors` needs `validate` in `derive`"),
        "{error}"
    );
    assert!(!inputs.path("gen").exists());

    config.derive = vec![Derive::Serde, Derive::Validate];
    rok_data_sqlc::generate(&config)?;
    assert_eq!(Derive::Validate.name(), "validate");
    assert_eq!(Extractor::ActixWeb.name(), "actix_web");
    Ok(())
}

/// `extractors = ["axum"]` adds `db/http/mod.rs` (the shared error body) and
/// `db/http/axum.rs`, declared by `db/mod.rs` with `axum` among the
/// dependency hints; without `extractors` there is no `db/http`.
#[test]
fn extractors_generate_the_http_module() -> Result<()> {
    use rok_data_sqlc::{Derive, Extractor};
    let inputs = Inputs::new()?;

    let mut config = inputs.config("plain");
    config.derive = vec![Derive::Serde, Derive::Validate];
    rok_data_sqlc::generate(&config)?;
    let plain = tree(&inputs.path("plain"))?;
    assert!(
        plain.keys().all(|path| !path.starts_with("http")),
        "{:?}",
        plain.keys()
    );
    assert!(!plain[Path::new("mod.rs")].contains("http"));
    assert!(!plain[Path::new("mod.rs")].contains("axum"));

    let mut config = inputs.config("axum");
    config.derive = vec![Derive::Serde, Derive::Validate];
    config.extractors = vec![Extractor::Axum];
    rok_data_sqlc::generate(&config)?;
    let generated = tree(&inputs.path("axum"))?;
    let http_mod = &generated[&Path::new("http").join("mod.rs")];
    assert!(http_mod.contains("pub mod axum;"), "{http_mod}");
    assert!(http_mod.contains("pub struct ErrorBody"), "{http_mod}");
    let axum = &generated[&Path::new("http").join("axum.rs")];
    assert!(
        axum.contains("pub struct ValidatedJson<T>(pub T);"),
        "{axum}"
    );
    let db_mod = &generated[Path::new("mod.rs")];
    assert!(db_mod.contains("pub mod http;"), "{db_mod}");
    assert!(db_mod.contains("//   axum = \"0.8\"\n"), "{db_mod}");
    assert!(
        db_mod.contains("//   serde_path_to_error = \"0.1\"\n"),
        "{db_mod}"
    );
    assert!(db_mod.contains("//   serde_json = \"1\"\n"), "{db_mod}");
    // Everything else is what the same run without `extractors` generates.
    for (path, text) in &plain {
        if path != Path::new("mod.rs") {
            assert_eq!(&generated[path], text, "{}", path.display());
        }
    }
    Ok(())
}

/// `http` is a reserved module name only when `extractors` generates
/// `db/http/`: without it, a table named `http` generates as in 0.2.1.
#[test]
fn a_table_named_http_is_reserved_only_with_extractors() -> Result<()> {
    use rok_data_sqlc::{Derive, Extractor};
    let dir = TempDir::new()?;
    fs::write(
        dir.path().join("schema.sql"),
        "CREATE TABLE http (id BIGSERIAL PRIMARY KEY, url TEXT NOT NULL);",
    )?;
    let mut config = Config::default();
    config.schema = vec![dir.path().join("schema.sql")];
    config.output = dir.path().join("plain");
    config.derive = vec![Derive::Serde, Derive::Validate];
    let report = rok_data_sqlc::generate(&config)?;
    assert!(
        report.table_failures.is_empty(),
        "{:?}",
        report.table_failures
    );
    let plain = tree(&dir.path().join("plain"))?;
    assert!(plain.contains_key(&Path::new("http").join("models.rs")));

    config.output = dir.path().join("extractors");
    config.extractors = vec![Extractor::Axum];
    let error = rok_data_sqlc::generate(&config).unwrap_err();
    let failures = &error.report().table_failures;
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0]
            .reason
            .contains("module name `http` is reserved for the generated `db/http/`"),
        "{}",
        failures[0].reason
    );
    Ok(())
}

/// `extractors = ["actix_web"]` adds `db/http/actix_web.rs` next to the
/// shared `db/http/mod.rs`, with `actix-web` among the dependency hints;
/// `["axum", "actix_web"]` generates both side by side, sharing `mod.rs`.
#[test]
fn the_actix_web_extractor_is_generated_alone_or_beside_axum() -> Result<()> {
    use rok_data_sqlc::{Derive, Extractor};
    let inputs = Inputs::new()?;

    let mut config = inputs.config("actix");
    config.derive = vec![Derive::Serde, Derive::Validate];
    config.extractors = vec![Extractor::ActixWeb];
    rok_data_sqlc::generate(&config)?;
    let actix = tree(&inputs.path("actix"))?;
    let http_mod = &actix[&Path::new("http").join("mod.rs")];
    assert!(http_mod.contains("pub mod actix_web;"), "{http_mod}");
    assert!(!http_mod.contains("pub mod axum;"), "{http_mod}");
    assert!(!actix.contains_key(&Path::new("http").join("axum.rs")));
    let extractor = &actix[&Path::new("http").join("actix_web.rs")];
    assert!(
        extractor.contains("pub struct ValidatedJson<T>(pub T);"),
        "{extractor}"
    );
    assert!(
        extractor.contains("impl ResponseError for BodyRejection"),
        "{extractor}"
    );
    let db_mod = &actix[Path::new("mod.rs")];
    assert!(db_mod.contains("pub mod http;"), "{db_mod}");
    assert!(db_mod.contains("//   actix-web = \"4\"\n"), "{db_mod}");
    assert!(!db_mod.contains("axum"), "{db_mod}");

    let mut config = inputs.config("both");
    config.derive = vec![Derive::Serde, Derive::Validate];
    config.extractors = vec![Extractor::ActixWeb, Extractor::Axum];
    rok_data_sqlc::generate(&config)?;
    let both = tree(&inputs.path("both"))?;
    let http_mod = &both[&Path::new("http").join("mod.rs")];
    assert!(
        http_mod.contains("pub mod actix_web;\npub mod axum;\n"),
        "{http_mod}"
    );
    assert_eq!(
        both[&Path::new("http").join("actix_web.rs")],
        *extractor,
        "the actix-web module does not depend on axum being generated"
    );
    let db_mod = &both[Path::new("mod.rs")];
    assert!(db_mod.contains("//   axum = \"0.8\"\n"), "{db_mod}");
    assert!(db_mod.contains("//   actix-web = \"4\"\n"), "{db_mod}");
    Ok(())
}
