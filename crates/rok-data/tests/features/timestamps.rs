//! The managed timestamp columns.

// The tests below need the chrono feature; without it the import would be
// unused.
#[cfg(feature = "chrono")]
use super::*;

#[cfg(feature = "chrono")]
mod timestamps_sql {
    use super::*;
    use rok_data::sqlx::types::chrono::{DateTime, Utc};

    #[derive(Debug, Clone, Model)]
    #[rok(timestamps)]
    pub(super) struct Note {
        #[rok(generated)]
        pub id: i64,
        pub body: String,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    #[test]
    fn timestamps_are_written_by_the_database() {
        assert_eq!(Note::CREATED_AT_COLUMN, Some("created_at"));
        assert_eq!(Note::UPDATED_AT_COLUMN, Some("updated_at"));
        let note = Note {
            id: 0,
            body: "b".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let sql = rok_data::__private::__insert_sql(&[note], false);
        assert_eq!(
            sql.as_str(),
            r#"INSERT INTO "notes" ("body", "created_at", "updated_at") VALUES ($1, now(), now()) RETURNING "id", "body", "created_at", "updated_at""#
        );
        assert_eq!(
            Note::update_all()
                .set(Note::BODY, "x")
                .filter(Note::ID.eq(1))
                .to_sql()
                .as_str(),
            r#"UPDATE "notes" SET "body" = $1, "updated_at" = now() WHERE "id" = $2"#
        );
        assert_eq!(
            Note::insert_builder()
                .set(Note::BODY, "x")
                .to_sql()
                .as_str(),
            r#"INSERT INTO "notes" ("body", "created_at", "updated_at") VALUES ($1, now(), now()) RETURNING "id", "body", "created_at", "updated_at""#
        );
    }
}

#[cfg(feature = "chrono")]
#[tokio::test]
async fn timestamps() {
    use rok_data::sqlx::types::chrono::{TimeZone, Utc};
    use timestamps_sql::Note;

    let Some(db) = setup().await else { return };
    db.execute(
        "CREATE TEMP TABLE notes (id BIGSERIAL PRIMARY KEY, body TEXT NOT NULL,
         created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL)",
    )
    .await
    .unwrap();
    let epoch = Utc.timestamp_opt(0, 0).unwrap();

    let note = Note {
        id: 0,
        body: "a".into(),
        created_at: epoch,
        updated_at: epoch,
    }
    .insert(&db)
    .await
    .unwrap();
    assert!(
        note.created_at > epoch,
        "set by the database, not the struct"
    );
    assert_eq!(note.created_at, note.updated_at);

    // `now()` is the transaction start time, so use separate statements.
    let mut edited = note.clone();
    edited.body = "b".into();
    edited.created_at = epoch;
    let saved = edited.update(&db).await.unwrap();
    assert_eq!(
        saved.created_at, note.created_at,
        "created_at is never updated"
    );
    assert!(saved.updated_at > note.updated_at);

    Note::filter(Note::ID.eq(note.id))
        .update()
        .set(Note::BODY, "c")
        .exec(&db)
        .await
        .unwrap();
    let reloaded = note.reload(&db).await.unwrap();
    assert!(reloaded.updated_at > saved.updated_at);

    let upserted = Note {
        body: "d".into(),
        ..reloaded.clone()
    }
    .upsert(&db)
    .await
    .unwrap();
    assert_eq!(upserted.created_at, note.created_at);
    assert!(upserted.updated_at > reloaded.updated_at);

    let built = Note::insert_builder()
        .set(Note::BODY, "e")
        .exec(&db)
        .await
        .unwrap();
    assert!(built.created_at > epoch);
}
