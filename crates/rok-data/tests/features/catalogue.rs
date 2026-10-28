//! The rule catalogue on input: the generated APIs' rules, codes and
//! messages.

use super::*;

#[derive(Debug, Clone, PartialEq, Input)]
struct Signup {
    #[rok(validate(url))]
    site: String,
    #[rok(validate(uuid(versions = [4])))]
    invite: String,
    #[rok(validate(ip(version = 4)))]
    from: Option<String>,
    #[rok(validate(alpha_numeric(dashes, underscores)))]
    handle: String,
    #[rok(validate(one_of(values = ["SA", "AE", "EG"])))]
    country: String,
    #[rok(validate(starts_with = "+"))]
    phone: String,
    #[rok(validate(hex_code))]
    color: String,
    #[rok(validate(iban))]
    account: Option<String>,
    #[rok(validate(credit_card(providers = ["visa"])))]
    card: Option<String>,
    #[rok(validate(coordinates))]
    location: Option<String>,
}

fn valid() -> Signup {
    Signup {
        site: "https://example.com/a?b=c".into(),
        invite: "8f14e45f-ceea-467a-9575-0e2a4c3d1b9e".into(),
        from: Some("10.0.0.1".into()),
        handle: "ann_b-1".into(),
        country: "SA".into(),
        phone: "+966".into(),
        color: "#a0f".into(),
        account: Some("DE89 3704 0044 0532 0130 00".into()),
        card: Some("4111 1111 1111 1111".into()),
        location: Some("(40.7, -74.0)".into()),
    }
}

#[test]
fn valid_values_pass() {
    assert!(valid().validate_fields().is_ok());
}

#[test]
fn every_rule_answers_its_code_and_message() {
    let bad = Signup {
        site: "not a url".into(),
        invite: "8f14e45f-ceea-167a-9575-0e2a4c3d1b9e".into(),
        from: Some("::1".into()),
        handle: "ann b".into(),
        country: "US".into(),
        phone: "966".into(),
        color: "red".into(),
        account: Some("DE00 0000".into()),
        card: Some("5555 5555 5555 4444".into()),
        location: Some("91,0".into()),
    };
    let rok_data::Error::Validation(errors) = bad.validate_fields().unwrap_err() else {
        panic!("expected validation errors");
    };
    let got: Vec<(&str, &str, &str)> = errors
        .errors()
        .iter()
        .map(|e| (e.field.as_str(), e.code, e.message.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("site", "url", "site must be a URL"),
            ("invite", "uuid", "invite must be a UUID"),
            ("from", "ipAddress", "from must be an IPv4 address"),
            (
                "handle",
                "alphaNumeric",
                "handle must contain only letters, digits, dashes and underscores"
            ),
            ("country", "in", "country must be one of SA, AE, EG"),
            ("phone", "startsWith", "phone must start with +"),
            ("color", "hexCode", "color must be a hex color code"),
            ("account", "iban", "account must be an IBAN"),
            ("card", "creditCard", "card must be a credit card number"),
            (
                "location",
                "coordinates",
                "location must be latitude and longitude coordinates"
            ),
        ]
    );
}

#[derive(Debug, Clone, PartialEq, Input)]
struct Code {
    #[rok(validate(min_length = 2, max_length = 4))]
    short: String,
    #[rok(validate(fixed_length = 1))]
    letter: String,
}

#[test]
fn character_counts_answer_the_length_codes() {
    let rok_data::Error::Validation(errors) = Code {
        short: "abcdé".into(),
        letter: "ab".into(),
    }
    .validate_fields()
    .unwrap_err() else {
        panic!("expected validation errors");
    };
    let got: Vec<(&str, &str)> = errors
        .errors()
        .iter()
        .map(|e| (e.code, e.message.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("maxLength", "short must be at most 4 characters"),
            ("fixedLength", "letter must be exactly 1 character"),
        ]
    );
}

#[cfg(feature = "regex")]
#[derive(Debug, Clone, PartialEq, Input)]
struct Contact {
    #[rok(validate(mobile(locales = ["ar-SA"], strict)))]
    phone: String,
    #[rok(validate(postal_code(countries = ["US", "DE"])))]
    zip: String,
    #[rok(validate(regex = "^[a-z]+-[0-9]+$"))]
    tag: String,
}

#[cfg(feature = "regex")]
#[test]
fn pattern_rules_check_their_tables() {
    let ok = Contact {
        phone: "+966512345678".into(),
        zip: "12345".into(),
        tag: "ab-12".into(),
    };
    assert!(ok.validate_fields().is_ok());
    let rok_data::Error::Validation(errors) = Contact {
        phone: "0512345678".into(),
        zip: "1234".into(),
        tag: "AB12".into(),
    }
    .validate_fields()
    .unwrap_err() else {
        panic!("expected validation errors");
    };
    let codes: Vec<&str> = errors.errors().iter().map(|e| e.code).collect();
    assert_eq!(codes, ["mobile", "postalCode", "regex"]);
}
