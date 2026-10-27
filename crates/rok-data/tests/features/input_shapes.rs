//! Input shapes: conditional rules, nested inputs, lists of them, enums.

use super::*;

#[derive(Debug, Clone, PartialEq, Input)]
struct Address {
    #[rok(transform(trim), validate(length(min = 1)))]
    city: String,
    #[rok(validate(length(min = 4, max = 10)))]
    zip: String,
}

#[derive(Debug, Clone, PartialEq, Input)]
struct Line {
    #[rok(validate(non_empty))]
    sku: String,
    #[rok(validate(range(min = 1)))]
    quantity: i32,
}

/// An order: a kind that decides what is required, a nested address,
/// lines, and a coupon checked only on weekends.
#[derive(Debug, Clone, PartialEq, Input)]
struct Order {
    kind: String,
    #[rok(validate(required_if(field = kind, equals = "company")))]
    company: Option<String>,
    #[rok(validate(required_unless(field = kind, equals = "pickup")))]
    phone: Option<String>,
    note: Option<String>,
    #[rok(validate(required_with(note)))]
    note_author: Option<String>,
    #[rok(nested)]
    address: Address,
    #[rok(nested)]
    billing: Option<Address>,
    #[rok(nested)]
    lines: Vec<Line>,
    weekend: bool,
    #[rok(when = on_weekends, validate(length(min = 5)))]
    coupon: String,
}

fn on_weekends(order: &Order) -> bool {
    order.weekend
}

fn order() -> Order {
    Order {
        kind: "person".into(),
        company: None,
        phone: Some("555".into()),
        note: None,
        note_author: None,
        address: Address {
            city: " Oslo ".into(),
            zip: "0150".into(),
        },
        billing: None,
        lines: vec![Line {
            sku: "A1".into(),
            quantity: 1,
        }],
        weekend: false,
        coupon: "x".into(),
    }
}

fn fields(error: rok_data::Error) -> Vec<(String, &'static str)> {
    let rok_data::Error::Validation(errors) = error else {
        panic!("expected validation errors, got {error:?}");
    };
    let mut fields: Vec<_> = errors
        .errors()
        .iter()
        .map(|e| (e.field.clone(), e.code))
        .collect();
    fields.sort();
    fields
}

#[test]
fn a_valid_order_passes_with_nested_transforms_applied() {
    let valid = order().validate_fields().unwrap();
    assert_eq!(valid.address.city, "Oslo");
}

#[test]
fn conditional_rules_follow_the_other_fields() {
    let mut company = order();
    company.kind = "company".into();
    company.phone = None;
    company.note = Some("ring twice".into());
    assert_eq!(
        fields(company.validate_fields().unwrap_err()),
        [
            ("company".to_owned(), "required"),
            ("note_author".to_owned(), "required"),
            ("phone".to_owned(), "required"),
        ]
    );
    let mut pickup = order();
    pickup.kind = "pickup".into();
    pickup.phone = None;
    assert!(pickup.validate_fields().is_ok(), "a pickup needs no phone");
}

#[test]
fn nested_errors_carry_their_paths() {
    let mut bad = order();
    bad.address.zip = "1".into();
    bad.billing = Some(Address {
        city: "  ".into(),
        zip: "0150".into(),
    });
    bad.lines.push(Line {
        sku: String::new(),
        quantity: 0,
    });
    assert_eq!(
        fields(bad.validate_fields().unwrap_err()),
        [
            ("address.zip".to_owned(), "length"),
            ("billing.city".to_owned(), "length"),
            ("lines.1.quantity".to_owned(), "range"),
            ("lines.1.sku".to_owned(), "non_empty"),
        ]
    );
}

#[test]
fn when_gates_a_fields_rules() {
    let mut weekend = order();
    weekend.weekend = true;
    assert_eq!(
        fields(weekend.validate_fields().unwrap_err()),
        [("coupon".to_owned(), "length")]
    );
}

#[derive(Debug, Clone, PartialEq, Input)]
struct Person {
    #[rok(validate(length(min = 1)))]
    name: String,
}

#[derive(Debug, Clone, PartialEq, Input)]
struct Company {
    #[rok(transform(uppercase), validate(length(min = 2)))]
    vat: String,
}

/// One body, two shapes.
#[derive(Debug, Clone, PartialEq, Input)]
enum Customer {
    Person(Person),
    Company(Company),
}

#[test]
fn an_enum_input_validates_its_variant() {
    let company = Customer::Company(Company { vat: "no1".into() })
        .validate_fields()
        .unwrap();
    assert_eq!(company, Customer::Company(Company { vat: "NO1".into() }));
    let nobody = Customer::Person(Person {
        name: String::new(),
    });
    assert_eq!(
        fields(nobody.validate_fields().unwrap_err()),
        [("name".to_owned(), "length")]
    );
}

/// A field type the schema can't describe.
#[cfg(feature = "json")]
#[derive(Debug, Clone, PartialEq)]
struct Opaque;

#[cfg(feature = "json")]
#[derive(Debug, Clone, PartialEq, Input)]
struct Listing {
    #[rok(validate(length(min = 2, max = 40)))]
    title: String,
    #[rok(validate(email))]
    contact: Option<String>,
    #[rok(validate(range(min = 0, max = 100)))]
    discount: i32,
    #[rok(validate(non_empty))]
    tags: Vec<String>,
    #[rok(nested)]
    address: Address,
    extra: Opaque,
}

#[cfg(feature = "json")]
#[test]
fn the_schema_says_the_types_and_rules() {
    use serde_json::json;
    let schema = Listing::json_schema();
    assert_eq!(
        schema,
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "title": {"type": "string", "minLength": 2, "maxLength": 40},
                "contact": {"type": ["string", "null"], "format": "email"},
                "discount": {"type": "integer", "minimum": 0.0, "maximum": 100.0},
                "tags": {"type": "array", "items": {"type": "string"}, "minItems": 1},
                "address": {
                    "type": "object",
                    "properties": {
                        "city": {"type": "string", "minLength": 1},
                        "zip": {"type": "string", "minLength": 4, "maxLength": 10}
                    },
                    "required": ["city", "zip"]
                },
                "extra": {}
            },
            "required": ["title", "discount", "tags", "address", "extra"]
        })
    );
    let customer = Customer::json_schema();
    assert_eq!(customer["oneOf"].as_array().map(Vec::len), Some(2));
}
