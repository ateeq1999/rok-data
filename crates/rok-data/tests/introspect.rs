//! Statements described without running them (`rok_data::introspect::describe`).
#![cfg(feature = "testing")]

use rok_data::introspect::{self, StatementColumn};
use rok_data::prelude::*;

const NOTES: &str = "CREATE TABLE notes (
    id BIGSERIAL PRIMARY KEY, body TEXT NOT NULL, pinned_at TIMESTAMPTZ, tags TEXT[] NOT NULL
)";

fn column(name: &str, type_name: &str, nullable: Option<bool>) -> StatementColumn {
    StatementColumn {
        name: name.into(),
        type_name: type_name.into(),
        nullable,
    }
}

#[rok_data::test]
async fn describe_answers_parameter_and_column_types(db: Db) {
    db.execute(NOTES).await.unwrap();
    let statement = introspect::describe(
        &db,
        "SELECT id, body, pinned_at, tags FROM notes WHERE id = $1 AND tags && $2",
    )
    .await
    .unwrap();
    assert_eq!(statement.params, ["INT8", "TEXT[]"]);
    assert_eq!(
        statement.columns,
        [
            column("id", "INT8", Some(false)),
            column("body", "TEXT", Some(false)),
            column("pinned_at", "TIMESTAMPTZ", Some(true)),
            column("tags", "TEXT[]", Some(false)),
        ]
    );
}

#[rok_data::test]
async fn describe_runs_nothing(db: Db) {
    db.execute(NOTES).await.unwrap();
    let statement = introspect::describe(
        &db,
        "INSERT INTO notes (body, tags) VALUES ($1, '{}') RETURNING id",
    )
    .await
    .unwrap();
    assert_eq!(statement.params, ["TEXT"]);
    let count: i64 = rok_data::raw("SELECT count(*) FROM notes")
        .scalar(&db)
        .await
        .unwrap();
    assert_eq!(count, 0, "the insert was only prepared");

    let error = introspect::describe(&db, "SELECT nope FROM notes").await;
    assert!(error.is_err(), "{error:?}");
}
