//! `#[derive(Input)]`: transforms, field rules and record rules on input.

use super::*;
use rok_data::ValidationErrors;

#[derive(Debug, Clone, PartialEq, Input)]
#[rok(validate_with = names_differ)]
struct SignUp {
    #[rok(transform(normalize_email), validate(email))]
    email: String,
    #[rok(transform(trim), validate(length(min = 2, max = 20)))]
    name: String,
    #[rok(transform(trim, uppercase), validate(non_empty))]
    nickname: Option<String>,
    #[rok(transform(custom = double), validate(range(min = 0, max = 10)))]
    age: i32,
}

fn double(age: i32) -> i32 {
    age * 2
}

fn names_differ(input: &SignUp) -> Result<(), ValidationErrors> {
    let mut errors = ValidationErrors::new();
    if input.nickname.as_deref() == Some(input.name.to_uppercase().as_str()) {
        errors.add("nickname", "different", "must differ from the name");
    }
    errors.into_result()
}

fn sign_up(email: &str, name: &str, nickname: Option<&str>, age: i32) -> SignUp {
    SignUp {
        email: email.into(),
        name: name.into(),
        nickname: nickname.map(Into::into),
        age,
    }
}

#[test]
fn transforms_run_before_the_rules_and_stay_in_the_result() {
    let valid = sign_up("  Ann@Example.COM ", "  ann  ", Some(" a "), 4)
        .validate_fields()
        .unwrap();
    assert_eq!(valid, sign_up("ann@example.com", "ann", Some("A"), 8));
}

#[test]
fn every_failure_is_collected() {
    let error = sign_up("nope", " a ", Some("   "), 6)
        .validate_fields()
        .unwrap_err();
    let rok_data::Error::Validation(errors) = error else {
        panic!("expected validation errors");
    };
    let mut codes: Vec<(&str, &str)> = errors
        .errors()
        .iter()
        .map(|e| (e.field.as_str(), e.code))
        .collect();
    codes.sort_unstable();
    assert_eq!(
        codes,
        [
            ("age", "range"),
            ("email", "email"),
            ("name", "length"),
            ("nickname", "non_empty"),
        ]
    );
}

#[test]
fn the_record_rule_runs_on_the_transformed_input() {
    let error = sign_up("a@b.io", "bob", Some(" bob "), 1)
        .validate_fields()
        .unwrap_err();
    assert!(
        error.to_string().contains("must differ from the name"),
        "{error}"
    );
}

#[tokio::test]
async fn validate_takes_a_session() {
    let Some(db) = setup().await else { return };
    let valid = sign_up("a@b.io", "ann", None, 1)
        .validate(&db)
        .await
        .unwrap();
    assert_eq!(valid.age, 2);
}

/// Messages: a rule's own, a provider's, the built-in one.
#[derive(Debug, Clone, PartialEq, Input)]
struct Profile {
    #[rok(validate(length(min = 3, message = "{field} needs {min}+ characters")))]
    handle: String,
    #[rok(validate(length(min = 2)))]
    bio: String,
    #[rok(validate(email(message = "that doesn't look like an email")))]
    email: String,
}

struct Shouting;

impl rok_data::validate::Messages for Shouting {
    fn template(&self, code: &str, _field: &str) -> Option<String> {
        (code == "length").then(|| "{field} TOO SHORT (MIN {min})".to_owned())
    }
}

#[test]
fn messages_come_from_the_rule_then_the_provider_then_the_default() {
    let error = Profile {
        handle: "ab".into(),
        bio: "x".into(),
        email: "nope".into(),
    }
    .validate_fields()
    .unwrap_err();
    let rok_data::Error::Validation(errors) = error else {
        panic!("expected validation errors");
    };
    let messages = |errors: &ValidationErrors| -> Vec<String> {
        errors.errors().iter().map(|e| e.message.clone()).collect()
    };
    assert_eq!(
        messages(&errors),
        [
            "handle needs 3+ characters",
            "length must be at least 2",
            "that doesn't look like an email",
        ]
    );
    let localized = errors.clone().localized(&Shouting);
    assert_eq!(
        messages(&localized),
        [
            "handle needs 3+ characters",
            "bio TOO SHORT (MIN 2)",
            "that doesn't look like an email",
        ]
    );
    #[cfg(feature = "json")]
    {
        use rok_data::validate::ErrorReporter;
        let report = rok_data::validate::DetailsReporter.report(&errors);
        assert_eq!(report["error"]["code"], "validation_failed");
        assert_eq!(report["error"]["details"]["bio"][0]["params"]["min"], 2);
    }
}
