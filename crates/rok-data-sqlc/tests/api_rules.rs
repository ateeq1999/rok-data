//! String rules (`[api.rules]`, VineJS names) through the library entry
//! point: what each rule puts on the create and update bodies, and what the
//! OpenAPI document learns of it. The rules' behaviour is tested by running
//! them in `tests/api-app`.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use rok_data_sqlc::{ApiConfig, ApiRule, Config, Derive, Extractor};
use tempfile::TempDir;

const SCHEMA: &str = "CREATE TABLE users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL,
    nickname TEXT,
    handle VARCHAR(40) NOT NULL DEFAULT 'anonymous',
    age INT
);";

struct Project {
    dir: TempDir,
}

impl Project {
    fn new(schema: &str) -> Result<Self> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("schema.sql"), schema)?;
        Ok(Project { dir })
    }

    fn path(&self, path: &str) -> PathBuf {
        self.dir.path().join(path)
    }

    fn read(&self, path: &str) -> Result<String> {
        Ok(fs::read_to_string(self.path(path))?)
    }

    /// The configuration `rok sqlc init` writes, with `rules` on `users`.
    fn config(&self, rules: &[(&str, &[&str])]) -> Config {
        let mut config = Config::default();
        config.schema = vec![self.path("schema.sql")];
        config.output = self.path("src/db");
        config.derive = vec![Derive::Serde, Derive::Validate];
        config.extractors = vec![Extractor::ActixWeb];
        let mut api = ApiConfig::new(self.path("src/features"));
        let columns: BTreeMap<String, Vec<ApiRule>> = rules
            .iter()
            .map(|(column, rules)| {
                let rules = rules.iter().map(|rule| ApiRule::from(*rule)).collect();
                ((*column).to_owned(), rules)
            })
            .collect();
        api.rules.insert("users".into(), columns);
        config.api = Some(api);
        config
    }

    /// Generates `config` and answers the users feature's `dto.rs`.
    fn dto(&self, config: &Config) -> Result<String> {
        rok_data_sqlc::generate(config)?;
        self.read("src/features/users/dto.rs")
    }
}

/// The struct `name` of a generated file: from its line to its closing
/// brace.
fn struct_block<'a>(text: &'a str, name: &str) -> &'a str {
    let start = text
        .find(&format!("pub struct {name} {{"))
        .unwrap_or_else(|| panic!("no {name} in\n{text}"));
    let end = start + text[start..].find("\n}\n").expect("the closing brace");
    &text[start..end]
}

/// `email` under VineJS's name, and the transforms `trim` and
/// `toLowerCase`, which run before the rules, so validation and the
/// database see the transformed value.
#[test]
fn email_with_transforms_on_both_bodies() -> Result<()> {
    let project = Project::new(SCHEMA)?;
    let config = project.config(&[("email", &["email", "trim", "toLowerCase"])]);
    let dto = project.dto(&config)?;

    let attr = "    #[rok(
        transform(trim, lowercase),
        validate(email(message = \"email must be an email address\"))
    )]";
    let create = struct_block(&dto, "CreateUserDto");
    assert!(
        create.contains(&format!(
            "    /// The `email` column.
    ///
    /// Transforms: `trim`, `toLowerCase`.
    /// Rules: `email`.
    #[schema(format = \"email\")]
{attr}
    pub email: String,"
        )),
        "{create}"
    );
    let update = struct_block(&dto, "UpdateUserDto");
    assert!(
        update.contains(&format!(
            "    #[schema(format = \"email\")]\n{attr}\n    pub email: Option<String>,"
        )),
        "{update}"
    );
    assert!(dto.contains("serde::Deserialize, rok_data::Input"), "{dto}");
    assert!(!dto.contains("validator"), "{dto}");
    Ok(())
}

/// Every other rule is the runtime catalogue's, next to the column's own
/// `length` rule; the OpenAPI document gets the keywords utoipa can carry,
/// and the rest in the field's description.
#[test]
fn other_rules_are_the_runtime_catalogue() -> Result<()> {
    let project = Project::new(SCHEMA)?;
    let config = project.config(&[
        ("handle", &["alphaNumeric(allowDashes)", "minLength(3)"]),
        ("nickname", &["in(SA, AE, EG)"]),
    ]);
    let dto = project.dto(&config)?;

    let create = struct_block(&dto, "CreateUserDto");
    assert!(
        create.contains(
            "    /// Rules: `alphaNumeric(allowDashes)`, `minLength(3)`.
    #[schema(min_length = 3)]
    #[rok(validate(
        length(max = 40, message = \"handle must be at most 40 characters\"),
        alpha_numeric(dashes),
        min_length = 3
    ))]
    pub handle: Option<String>,"
        ),
        "{create}"
    );
    assert!(
        create.contains(
            "    #[rok(validate(one_of(values = [\"SA\", \"AE\", \"EG\"])))]\n    pub nickname: Option<String>,"
        ),
        "{create}"
    );
    // No generated check functions, no helpers.
    assert!(!dto.contains("\nfn handle_"), "{dto}");
    assert!(!dto.contains("shared::validation::check"), "{dto}");
    let shared = project.read("src/shared/validation.rs")?;
    assert!(!shared.contains("pub fn check("), "{shared}");
    // No regular expression, so no `regex` feature.
    let db_mod = project.read("src/db/mod.rs")?;
    assert!(!db_mod.contains("regex"), "{db_mod}");
    Ok(())
}

/// A regular expression (`regex`, `mobile`, `postalCode`) needs rok-data's
/// `regex` feature; the data layer's hints name it.
#[test]
fn patterns_need_the_regex_feature() -> Result<()> {
    let project = Project::new(SCHEMA)?;
    let mut config = project.config(&[("handle", &["mobile(ar-SA, strictMode)"])]);
    config
        .api
        .as_mut()
        .expect("the API")
        .rules
        .get_mut("users")
        .expect("the users rules")
        .insert(
            "nickname".into(),
            vec![ApiRule::Regex("^[a-z0-9-]+$".into())],
        );
    let dto = project.dto(&config)?;
    let create = struct_block(&dto, "CreateUserDto");
    assert!(
        create.contains("mobile(locales = [\"ar-SA\"], strict)"),
        "{create}"
    );
    assert!(
        create.contains("    #[schema(pattern = \"^[a-z0-9-]+$\")]\n    #[rok(validate(regex = \"^[a-z0-9-]+$\"))]"),
        "{create}"
    );
    let db_mod = project.read("src/db/mod.rs")?;
    assert!(db_mod.contains("\"regex\"] }"), "{db_mod}");
    assert!(!db_mod.contains("//   regex = "), "{db_mod}");
    Ok(())
}

/// A nullable column's transforms keep `null` meaning "set to `NULL`" in
/// the update body, and an optional create field stays optional.
#[test]
fn transforms_keep_null_and_defaults() -> Result<()> {
    let project = Project::new(SCHEMA)?;
    let config = project.config(&[("nickname", &["trim", "normalizeEmail"])]);
    let dto = project.dto(&config)?;
    let transform = "    #[rok(transform(trim, normalize_email))]\n";
    assert!(
        struct_block(&dto, "CreateUserDto")
            .contains(&format!("{transform}    pub nickname: Option<String>,")),
        "{dto}"
    );
    assert!(
        struct_block(&dto, "UpdateUserDto").contains(&format!(
            "    #[serde(deserialize_with = \"crate::db::nullable::deserialize\")]\n{transform}    pub nickname: Option<Option<String>>,"
        )),
        "{dto}"
    );
    // The transforms are the runtime's: nothing to share.
    assert!(!dto.contains("_transformed"), "{dto}");
    let shared = project.read("src/shared/validation.rs")?;
    assert!(!shared.contains("Transformed"), "{shared}");
    Ok(())
}

/// Without the OpenAPI document, no `schema` attribute.
#[test]
fn rules_without_the_document() -> Result<()> {
    let project = Project::new(SCHEMA)?;
    let mut config = project.config(&[("email", &["email", "maxLength(80)"])]);
    config.api.as_mut().expect("the API").openapi = false;
    let dto = project.dto(&config)?;
    assert!(!dto.contains("#[schema("), "{dto}");
    assert!(dto.contains("max_length = 80"), "{dto}");
    Ok(())
}
/// Every rule at once, on long names, is laid out as rustfmt lays it out.
#[test]
fn every_rule_is_rustfmt_clean() -> Result<()> {
    let project = Project::new(
        "CREATE TABLE very_long_named_customer_contact_records (
    id UUID PRIMARY KEY,
    the_primary_contact_email_address_of_this_customer TEXT NOT NULL,
    a TEXT, b TEXT, c TEXT, d TEXT, e TEXT, f TEXT, g TEXT
);",
    )?;
    let mut config = project.config(&[]);
    let long = "the_primary_contact_email_address_of_this_customer";
    let every = [
        "trim",
        "toUpperCase",
        "toLowerCase",
        "normalizeEmail",
        "email",
        "url",
        "uuid(1, 2, 3, 4, 5, 6, 7, 8)",
        "ulid",
        "ipAddress(4)",
        "mobile(ar-SA, ar-AE, ar-EG, en-US, strictMode)",
        "alphaNumeric(allowSpaces, allowDashes, allowUnderscores)",
        "ascii",
        "notIn(alpha, beta, gamma, delta, epsilon, zeta, eta, theta, iota, kappa, lambda, mu, nu, xi)",
        "startsWith(x)",
        "endsWith(y)",
        "minLength(1)",
        "maxLength(4294967295)",
        "fixedLength(10)",
        "hexCode",
        "iban",
        "jwt",
        "postalCode(SA, US, GB, CA)",
        "creditCard(amex, visa, mastercard, dinersclub)",
        "coordinates",
    ];
    let rules: Vec<(&str, Vec<ApiRule>)> = vec![
        (long, every.into_iter().map(ApiRule::from).collect()),
        (
            "a",
            vec![ApiRule::In(vec!["a long value, with a comma".into(); 3])],
        ),
        ("b", vec![ApiRule::Regex("^\"quoted\"\\d+$".into())]),
        (
            "c",
            vec![ApiRule::from("alpha"), ApiRule::from("creditCard")],
        ),
        ("d", vec![ApiRule::from("ipAddress")]),
        ("e", vec![ApiRule::StartsWith("a, b".into())]),
        ("f", vec![ApiRule::from("toUpperCase")]),
        (
            "g",
            vec![ApiRule::from(
                "in(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27)",
            )],
        ),
    ];
    let api = config.api.as_mut().expect("the API");
    api.rules.clear();
    api.rules.insert(
        "very_long_named_customer_contact_records".into(),
        rules
            .into_iter()
            .map(|(column, rules)| (column.to_owned(), rules))
            .collect(),
    );
    rok_data_sqlc::generate(&config)?;
    let dto = project.path("src/features/very_long_named_customer_contact_records/dto.rs");
    let output = std::process::Command::new("rustfmt")
        .args(["--edition", "2024", "--check"])
        .arg(&dto)
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}

/// `url` is VineJS's (validator.js's `isURL`), which takes a value without
/// a scheme: the runtime's rule, with no `uri` format.
#[test]
fn url_is_the_runtime_rule() -> Result<()> {
    let project = Project::new(SCHEMA)?;
    let config = project.config(&[("nickname", &["url"])]);
    let dto = project.dto(&config)?;
    assert!(
        dto.contains("    #[rok(validate(url))]\n    pub nickname: Option<String>,"),
        "{dto}"
    );
    assert!(!dto.contains("format = \"uri\""), "{dto}");
    Ok(())
}
