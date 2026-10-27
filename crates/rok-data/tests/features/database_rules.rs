//! Database rules on input: `unique`, `exists`, custom ones, run against
//! the session `validate` takes.

use super::*;
use rok_data::{Journey, ValidationErrors};

#[derive(Debug, Clone, PartialEq, Model)]
#[rok(table = "orgs_dr")]
struct Org {
    #[rok(generated)]
    id: i64,
    name: String,
    active: bool,
}

#[derive(Debug, Clone, PartialEq, Model)]
#[rok(table = "accounts_dr")]
struct Account {
    #[rok(generated)]
    id: i64,
    email: String,
    org_id: i64,
}

const TABLES: &str = r#"
CREATE TEMP TABLE orgs_dr (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL, active BOOLEAN NOT NULL);
CREATE TEMP TABLE accounts_dr (
    id BIGSERIAL PRIMARY KEY, email TEXT NOT NULL, org_id BIGINT NOT NULL REFERENCES orgs_dr (id)
);
"#;

/// Creating an account.
#[derive(Debug, Clone, PartialEq, Input)]
#[rok(validate_with_database = not_reserved)]
struct NewAccount {
    #[rok(
        transform(normalize_email),
        validate(email, unique(Account::EMAIL, case_insensitive))
    )]
    email: String,
    #[rok(validate(exists(Org::ID, scope = active_orgs)))]
    org_id: i64,
    #[rok(validate(database = handle_free))]
    handle: Option<String>,
}

/// Updating one: its own row doesn't collide with itself.
#[derive(Debug, Clone, PartialEq, Input)]
struct EditAccount {
    id: i64,
    #[rok(validate(unique(Account::EMAIL, except = id)))]
    email: String,
}

fn active_orgs(_input: &NewAccount) -> Expr<Org> {
    Org::ACTIVE.eq(true)
}

async fn handle_free(handle: &Option<String>, _session: Journey<'_>) -> Result<(), String> {
    match handle.as_deref() {
        Some("admin") => Err("handle is reserved".into()),
        _ => Ok(()),
    }
}

async fn not_reserved(input: &NewAccount, session: Journey<'_>) -> Result<(), ValidationErrors> {
    let _ = session;
    let mut errors = ValidationErrors::new();
    if input.email.starts_with("root@") {
        errors.add("email", "reserved", "email is reserved");
    }
    errors.into_result()
}

fn codes(error: rok_data::Error) -> Vec<(String, &'static str)> {
    let rok_data::Error::Validation(errors) = error else {
        panic!("expected validation errors, got {error:?}");
    };
    let mut codes: Vec<_> = errors
        .errors()
        .iter()
        .map(|e| (e.field.clone(), e.code))
        .collect();
    codes.sort();
    codes
}

async fn seeded(db: &Db) -> (Org, Org, Account) {
    db.execute(TABLES).await.unwrap();
    let org = |name: &str, active: bool| {
        Org::insert_builder()
            .set(Org::NAME, name.to_owned())
            .set(Org::ACTIVE, active)
            .exec(db)
    };
    let live = org("live", true).await.unwrap();
    let gone = org("gone", false).await.unwrap();
    let ann = Account::insert_builder()
        .set(Account::EMAIL, "ann@x.io")
        .set(Account::ORG_ID, live.id)
        .exec(db)
        .await
        .unwrap();
    (live, gone, ann)
}

fn new_account(email: &str, org_id: i64, handle: Option<&str>) -> NewAccount {
    NewAccount {
        email: email.into(),
        org_id,
        handle: handle.map(Into::into),
    }
}

#[tokio::test]
async fn database_rules_check_the_rows() {
    let Some(db) = setup().await else { return };
    let (live, gone, ann) = seeded(&db).await;

    // A fresh email in a live org passes, transformed.
    let ok = new_account(" Bob@X.io ", live.id, Some("bob"))
        .validate(&db)
        .await
        .unwrap();
    assert_eq!(ok.email, "bob@x.io");

    // Taken (ignoring case), an inactive org, a reserved handle.
    let error = new_account("ANN@x.io", gone.id, Some("admin"))
        .validate(&db)
        .await
        .unwrap_err();
    assert_eq!(
        codes(error),
        [
            ("email".to_owned(), "unique"),
            ("handle".to_owned(), "database"),
            ("org_id".to_owned(), "exists"),
        ]
    );

    // A field whose field rule failed never reaches the database rules.
    let error = new_account("not an email", 999, None)
        .validate(&db)
        .await
        .unwrap_err();
    assert_eq!(
        codes(error),
        [
            ("email".to_owned(), "email"),
            ("org_id".to_owned(), "exists")
        ]
    );

    // The record-level database rule.
    let error = new_account("root@x.io", live.id, None)
        .validate(&db)
        .await
        .unwrap_err();
    assert_eq!(codes(error), [("email".to_owned(), "reserved")]);

    // `except`: the row being edited doesn't collide with itself.
    let same = EditAccount {
        id: ann.id,
        email: "ann@x.io".into(),
    };
    assert!(same.validate(&db).await.is_ok());
    let other = EditAccount {
        id: ann.id + 1,
        email: "ann@x.io".into(),
    };
    assert_eq!(
        codes(other.validate(&db).await.unwrap_err()),
        [("email".to_owned(), "unique")]
    );
}

#[tokio::test]
async fn validation_runs_inside_a_transaction() {
    let Some(db) = setup().await else { return };
    let (live, _, _) = seeded(&db).await;
    let mut tx = db.begin().await.unwrap();
    Account::insert_builder()
        .set(Account::EMAIL, "tx@x.io")
        .set(Account::ORG_ID, live.id)
        .exec(&mut tx)
        .await
        .unwrap();
    // Inside the transaction, its own uncommitted row counts.
    let error = new_account("tx@x.io", live.id, None)
        .validate(&mut tx)
        .await
        .unwrap_err();
    assert_eq!(codes(error), [("email".to_owned(), "unique")]);
    tx.rollback().await.unwrap();
}
