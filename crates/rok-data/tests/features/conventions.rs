//! The inferred conventions: managed timestamps and soft deletes, and the
//! opt-outs that turn them off (workspace ADR 0015).
//!
//! The SQL-shape tests always run; the database test needs `DATABASE_URL`
//! (see `tests/postgres.rs`).

use super::*;

/// The field names are the signal: `created_at` and `updated_at` are managed
/// timestamps.
#[derive(Debug, Clone, PartialEq, Model)]
struct Stamp {
    #[rok(generated)]
    id: i64,
    body: String,
    created_at: String,
    updated_at: String,
}

/// The opt-out: the timestamp fields stay ordinary columns.
#[derive(Debug, Clone, PartialEq, Model)]
#[rok(no_timestamps)]
struct PlainStamp {
    #[rok(generated)]
    id: i64,
    body: String,
    created_at: String,
    updated_at: String,
}

/// The explicit marker still works, and means the same thing.
#[derive(Debug, Clone, PartialEq, Model)]
#[rok(timestamps)]
struct MarkedStamp {
    #[rok(generated)]
    id: i64,
    body: String,
    created_at: String,
    updated_at: String,
}

/// A `deleted_at` field is a soft-delete column.
#[derive(Debug, Clone, PartialEq, Model)]
struct Ticket {
    #[rok(generated)]
    id: i64,
    body: String,
    deleted_at: Option<String>,
}

/// The opt-out: the `deleted_at` field stays an ordinary column.
#[derive(Debug, Clone, PartialEq, Model)]
#[rok(no_soft_delete)]
struct PlainTicket {
    #[rok(generated)]
    id: i64,
    body: String,
    deleted_at: Option<String>,
}

/// The explicit marker still works, and means the same thing.
#[derive(Debug, Clone, PartialEq, Model)]
#[rok(soft_delete)]
struct MarkedTicket {
    #[rok(generated)]
    id: i64,
    body: String,
    deleted_at: Option<String>,
}

/// One timestamp column alone is not the convention: the model would manage
/// half a pair.
#[derive(Debug, Clone, PartialEq, Model)]
struct HalfStamp {
    #[rok(generated)]
    id: i64,
    body: String,
    created_at: String,
}

fn stamp() -> Stamp {
    Stamp {
        id: 0,
        body: "b".into(),
        created_at: String::new(),
        updated_at: String::new(),
    }
}

#[test]
fn the_field_names_infer_the_managed_timestamps() {
    assert_eq!(Stamp::CREATED_AT_COLUMN, Some("created_at"));
    assert_eq!(Stamp::UPDATED_AT_COLUMN, Some("updated_at"));
    assert_eq!(
        rok_data::__private::__insert_sql(&[stamp()], false).as_str(),
        r#"INSERT INTO "stamps" ("body", "created_at", "updated_at") VALUES ($1, now(), now()) RETURNING "id", "body", "created_at", "updated_at""#
    );
    assert_eq!(
        Stamp::update_all()
            .set(Stamp::BODY, "x")
            .filter(Stamp::ID.eq(1))
            .to_sql()
            .as_str(),
        r#"UPDATE "stamps" SET "body" = $1, "updated_at" = now() WHERE "id" = $2"#
    );
    assert_eq!(
        Stamp::insert_builder()
            .set(Stamp::BODY, "x")
            .to_sql()
            .as_str(),
        r#"INSERT INTO "stamps" ("body", "created_at", "updated_at") VALUES ($1, now(), now()) RETURNING "id", "body", "created_at", "updated_at""#
    );
}

#[test]
fn one_timestamp_column_alone_is_not_the_convention() {
    assert_eq!(HalfStamp::CREATED_AT_COLUMN, None);
    assert_eq!(HalfStamp::UPDATED_AT_COLUMN, None);
    assert!(
        !rok_data::__private::__insert_sql(
            &[HalfStamp {
                id: 0,
                body: "b".into(),
                created_at: String::new(),
            }],
            false
        )
        .as_str()
        .contains("now()")
    );
}

#[test]
fn the_opt_out_keeps_the_timestamp_fields_ordinary() {
    assert_eq!(PlainStamp::CREATED_AT_COLUMN, None);
    assert_eq!(PlainStamp::UPDATED_AT_COLUMN, None);
    let plain = PlainStamp {
        id: 0,
        body: "b".into(),
        created_at: "c".into(),
        updated_at: "u".into(),
    };
    assert_eq!(
        rok_data::__private::__insert_sql(&[plain], false).as_str(),
        r#"INSERT INTO "plain_stamps" ("body", "created_at", "updated_at") VALUES ($1, $2, $3) RETURNING "id", "body", "created_at", "updated_at""#
    );
    assert_eq!(
        PlainStamp::update_all()
            .set(PlainStamp::BODY, "x")
            .filter(PlainStamp::ID.eq(1))
            .to_sql()
            .as_str(),
        r#"UPDATE "plain_stamps" SET "body" = $1 WHERE "id" = $2"#
    );
}

#[test]
fn the_explicit_marker_still_works() {
    assert_eq!(MarkedStamp::CREATED_AT_COLUMN, Some("created_at"));
    assert_eq!(MarkedStamp::UPDATED_AT_COLUMN, Some("updated_at"));
    assert_eq!(MarkedTicket::DELETED_AT_COLUMN, Some("deleted_at"));
    assert_eq!(
        MarkedStamp::update_all()
            .set(MarkedStamp::BODY, "x")
            .filter(MarkedStamp::ID.eq(1))
            .to_sql()
            .as_str(),
        r#"UPDATE "marked_stamps" SET "body" = $1, "updated_at" = now() WHERE "id" = $2"#
    );
}

#[test]
fn the_deleted_at_field_infers_the_soft_delete() {
    assert_eq!(Ticket::DELETED_AT_COLUMN, Some("deleted_at"));
    // Every read leaves the soft-deleted rows out.
    assert_eq!(
        Ticket::query().to_sql().as_str(),
        r#"SELECT "id", "body", "deleted_at" FROM "tickets" WHERE "deleted_at" IS NULL"#
    );
    // And a delete marks the row instead of removing it.
    assert_eq!(
        Ticket::filter(Ticket::ID.eq(1)).delete_sql().as_str(),
        r#"UPDATE "tickets" SET "deleted_at" = now() WHERE "id" = $1 AND "deleted_at" IS NULL"#
    );
}

#[test]
fn the_opt_out_keeps_the_deleted_at_field_ordinary() {
    assert_eq!(PlainTicket::DELETED_AT_COLUMN, None);
    assert_eq!(
        PlainTicket::query().to_sql().as_str(),
        r#"SELECT "id", "body", "deleted_at" FROM "plain_tickets""#
    );
    assert_eq!(
        PlainTicket::filter(PlainTicket::ID.eq(1))
            .delete_sql()
            .as_str(),
        r#"DELETE FROM "plain_tickets" WHERE "id" = $1"#
    );
}

/// The database test needs a real timestamp type.
#[cfg(feature = "chrono")]
mod against_postgres {
    use super::*;
    use rok_data::sqlx::types::chrono::{DateTime, TimeZone, Utc};

    #[derive(Debug, Clone, Model)]
    struct Entry {
        #[rok(generated)]
        id: i64,
        body: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Model)]
    #[rok(no_timestamps)]
    struct PlainEntry {
        #[rok(generated)]
        id: i64,
        body: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Model)]
    struct Note {
        #[rok(generated)]
        id: i64,
        body: String,
        deleted_at: Option<DateTime<Utc>>,
    }

    #[tokio::test]
    async fn the_inferred_conventions_reach_the_database() {
        let Some(db) = setup().await else { return };
        db.execute(
            "CREATE TEMP TABLE entries (id BIGSERIAL PRIMARY KEY, body TEXT NOT NULL,
             created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL);
             CREATE TEMP TABLE plain_entries (id BIGSERIAL PRIMARY KEY, body TEXT NOT NULL,
             created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL);
             CREATE TEMP TABLE notes (id BIGSERIAL PRIMARY KEY, body TEXT NOT NULL,
             deleted_at TIMESTAMPTZ)",
        )
        .await
        .unwrap();
        let epoch = Utc.timestamp_opt(0, 0).unwrap();

        // The inferred timestamps are written by the database, not the struct.
        let entry = Entry {
            id: 0,
            body: "a".into(),
            created_at: epoch,
            updated_at: epoch,
        }
        .insert(&db)
        .await
        .unwrap();
        assert!(entry.created_at > epoch, "set by the database");
        assert_eq!(entry.created_at, entry.updated_at);

        // The opt-out keeps the fields ordinary: the struct's values are
        // written as they are.
        let plain = PlainEntry {
            id: 0,
            body: "a".into(),
            created_at: epoch,
            updated_at: epoch,
        }
        .insert(&db)
        .await
        .unwrap();
        assert_eq!(plain.created_at, epoch);
        assert_eq!(plain.updated_at, epoch);

        // The inferred soft delete hides the row from every read.
        let note = Note {
            id: 0,
            body: "a".into(),
            deleted_at: None,
        }
        .insert(&db)
        .await
        .unwrap();
        assert_eq!(Note::count(&db).await.unwrap(), 1);
        note.delete(&db).await.unwrap();
        assert_eq!(Note::count(&db).await.unwrap(), 0);
        assert_eq!(Note::query().with_trashed().count(&db).await.unwrap(), 1);
        note.restore(&db).await.unwrap();
        assert_eq!(Note::count(&db).await.unwrap(), 1);
    }
}
