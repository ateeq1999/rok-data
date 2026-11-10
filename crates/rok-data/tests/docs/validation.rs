//! `docs/validation.md`'s samples, compiled.

use rok_data::prelude::*;
use rok_data::{Journey, ValidationErrors};

#[derive(Debug, Clone, Model)]
struct User {
    #[rok(generated)]
    id: i64,
    email: String,
    org_id: i64,
}

#[derive(Debug, Clone, Model)]
struct Org {
    #[rok(generated)]
    id: i64,
    active: bool,
}

// an input: transforms, then field rules
#[derive(Debug, Clone, Input)]
struct SignUp {
    #[rok(transform(trim, normalize_email), validate(email))]
    email: String,
    #[rok(transform(trim), validate(length(min = 2, max = 40)))]
    name: String,
    // `Option` is optional: its rules run only on `Some`
    #[rok(validate(url))]
    website: Option<String>,
}

async fn sign_up(db: Db, body: SignUp) -> rok_data::Result<()> {
    // the transformed input, or `Error::Validation` with every failure
    let input = body.validate(&db).await?;

    // no database rules to run, or a test: no session needed
    let input = input.validate_fields()?;
    Ok(())
}

fn report(error: rok_data::Error) {
    if let rok_data::Error::Validation(errors) = error {
        for failure in errors.errors() {
            // the dotted path (`lines.2.sku`), the rule's code, its message
            println!("{} / {} / {}", failure.field, failure.code, failure.message);
        }
    }
}

// database rules: checked in one statement, for fields whose field rules passed
#[derive(Debug, Clone, Input)]
#[rok(validate_with_database = not_reserved)]
struct NewUser {
    #[rok(
        transform(trim, normalize_email),
        validate(email, unique(User::EMAIL, case_insensitive))
    )]
    email: String,
    #[rok(validate(exists(Org::ID, scope = active_orgs)))]
    org_id: i64,
    #[rok(validate(database = handle_free))]
    handle: Option<String>,
}

fn active_orgs(_input: &NewUser) -> Expr<Org> {
    Org::ACTIVE.eq(true)
}

async fn handle_free(handle: &Option<String>, session: Journey<'_>) -> Result<(), String> {
    match handle.as_deref() {
        Some("admin") => Err("that handle is reserved".into()),
        _ => Ok(()),
    }
}

async fn not_reserved(input: &NewUser, session: Journey<'_>) -> Result<(), ValidationErrors> {
    let mut errors = ValidationErrors::new();
    if input.email.starts_with("root@") {
        errors.add("email", "reserved", "that email is reserved");
    }
    errors.into_result()
}

// updating a row: it doesn't collide with itself
#[derive(Debug, Clone, Input)]
struct EditUser {
    id: i64,
    #[rok(validate(unique(User::EMAIL, except = id)))]
    email: String,
}

async fn inside_a_transaction(db: Db, body: NewUser) -> rok_data::Result<()> {
    let mut tx = db.begin().await?;
    let input = body.validate(&mut tx).await?;
    // … insert the rows on `tx` …
    tx.commit().await?;
    Ok(())
}

// who is asking: what a context-aware rule reads
struct Caller {
    org_id: i64,
}

#[derive(Debug, Clone, Input)]
#[rok(context = Caller)]
struct MoveUser {
    #[rok(validate(exists(User::ID, scope = in_callers_org)))]
    user_id: i64,
    #[rok(validate(database = not_own_org))]
    to_org: i64,
}

fn in_callers_org(_input: &MoveUser, caller: &Caller) -> Expr<User> {
    User::ORG_ID.eq(caller.org_id)
}

async fn not_own_org(to_org: &i64, session: Journey<'_>, caller: &Caller) -> Result<(), String> {
    if *to_org == caller.org_id {
        Err("the user is already in that org".into())
    } else {
        Ok(())
    }
}

async fn move_user(db: Db, body: MoveUser, caller: Caller) -> rok_data::Result<()> {
    let input = body.validate_with(&db, &caller).await?;
    Ok(())
}

// conditional rules, nesting and a record rule
#[derive(Debug, Clone, Input)]
#[rok(validate_with = names_differ)]
struct Order {
    kind: String,
    #[rok(validate(required_if(field = kind, equals = "company")))]
    company: Option<String>,
    #[rok(validate(required_unless(field = kind, equals = "pickup")))]
    phone: Option<String>,
    weekend: bool,
    #[rok(when = on_weekends, validate(length(min = 5)))]
    coupon: String,
    #[rok(nested)]
    address: Address,
    #[rok(nested)]
    lines: Vec<Line>,
}

#[derive(Debug, Clone, Input)]
struct Address {
    #[rok(transform(trim), validate(length(min = 1)))]
    city: String,
}

#[derive(Debug, Clone, Input)]
struct Line {
    #[rok(validate(non_empty))]
    sku: String,
    #[rok(validate(range(min = 1)))]
    quantity: i32,
}

fn on_weekends(order: &Order) -> bool {
    order.weekend
}

fn names_differ(order: &Order) -> Result<(), ValidationErrors> {
    let mut errors = ValidationErrors::new();
    if order.company.as_deref() == Some(order.address.city.as_str()) {
        errors.add(
            "company",
            "different",
            "the company can't be named after the city",
        );
    }
    errors.into_result()
}

// one body, two shapes: the variant's rules run
#[derive(Debug, Clone, Input)]
enum Customer {
    Person(Person),
    Company(Company),
}

#[derive(Debug, Clone, Input)]
struct Person {
    #[rok(validate(length(min = 1)))]
    name: String,
}

#[derive(Debug, Clone, Input)]
struct Company {
    #[rok(transform(uppercase), validate(length(min = 2)))]
    vat: String,
}

// a rule's own message, with the rule's arguments as placeholders
#[derive(Debug, Clone, Input)]
struct Profile {
    #[rok(validate(length(min = 3, message = "{field} needs {min}+ characters")))]
    handle: String,
    #[rok(validate(custom(function = not_admin, code = "reserved")))]
    nickname: String,
}

fn not_admin(nickname: &str) -> Result<(), String> {
    if nickname == "admin" {
        Err("that nickname is reserved".into())
    } else {
        Ok(())
    }
}

// every message in one place, keyed by the rule's code
struct Swahili;

impl rok_data::validate::Messages for Swahili {
    fn template(&self, code: &str, field: &str) -> Option<String> {
        match code {
            "length" => Some("{field} ni fupi mno (angalau {min})".to_owned()),
            "email" => Some("barua pepe si sahihi".to_owned()),
            _ => None,
        }
    }
}

fn translated(errors: ValidationErrors) -> ValidationErrors {
    errors.localized(&Swahili)
}

#[cfg(feature = "json")]
fn answered(errors: &ValidationErrors) -> serde_json::Value {
    use rok_data::validate::{DetailsReporter, ErrorReporter};

    // {"error": {"code": "validation_failed", "message": …,
    //            "details": {"email": [{"code": "email", "message": …, "params": {}}]}}}
    DetailsReporter.report(errors)
}

#[cfg(feature = "json")]
fn documented() -> serde_json::Value {
    // JSON Schema 2020-12: types, `required`, and the rules a schema can say
    SignUp::json_schema()
}
