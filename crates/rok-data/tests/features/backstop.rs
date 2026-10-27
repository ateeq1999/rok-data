//! The constraint backstop: a unique or foreign-key violation on a model
//! write reads as a field error.

use super::*;

#[derive(Debug, Clone, PartialEq, Model)]
#[rok(table = "members_bs")]
struct Member {
    #[rok(generated)]
    id: i64,
    email: String,
    team_id: Option<i64>,
}

const TABLES: &str = r#"
CREATE TEMP TABLE teams_bs (id BIGSERIAL PRIMARY KEY);
CREATE TEMP TABLE members_bs (
    id BIGSERIAL PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    team_id BIGINT REFERENCES teams_bs (id)
);
"#;

fn member(email: &str, team_id: Option<i64>) -> Member {
    Member {
        id: 0,
        email: email.into(),
        team_id,
    }
}

#[tokio::test]
async fn a_violation_on_a_write_names_the_field() {
    let Some(db) = setup().await else { return };
    db.execute(TABLES).await.unwrap();
    member("a@x.io", None).insert(&db).await.unwrap();

    let taken = member("a@x.io", None).insert(&db).await.unwrap_err();
    let rok_data::Error::Validation(errors) = &taken else {
        panic!("expected a validation error, got {taken:?}");
    };
    let first = &errors.errors()[0];
    assert_eq!((first.field.as_str(), first.code), ("email", "unique"));
    assert_eq!(first.message, "email is already taken");
    assert!(taken.is_unique_violation());

    let missing = member("b@x.io", Some(999)).insert(&db).await.unwrap_err();
    let rok_data::Error::Validation(errors) = &missing else {
        panic!("expected a validation error, got {missing:?}");
    };
    let first = &errors.errors()[0];
    assert_eq!((first.field.as_str(), first.code), ("team_id", "exists"));
    assert!(first.message.contains("teams_bs"), "{}", first.message);
    assert!(missing.is_foreign_key_violation());
}
