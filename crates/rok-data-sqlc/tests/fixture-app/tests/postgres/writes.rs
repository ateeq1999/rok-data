//! Inserts, partial updates, deletes, transactions, and JSON round trips.

use super::*;

pub(super) async fn insert_applies_defaults_and_explicit_nulls(pool: &PgPool) {
    let defaults = PatchTarget::create(
        pool,
        NewPatchTarget {
            label: "defaults".to_owned(),
            note: None,
            score: None,
            tag: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(defaults.score, 0, "omitted score uses DEFAULT 0");
    assert_eq!(
        defaults.tag.as_deref(),
        Some("none"),
        "omitted tag uses DEFAULT 'none'"
    );
    assert_eq!(defaults.note, None);
    assert_eq!(
        defaults.doubled,
        Some(0),
        "generated column is computed by the database"
    );

    let explicit = PatchTarget::create(
        pool,
        NewPatchTarget {
            label: "explicit".to_owned(),
            note: Some("n".to_owned()),
            score: Some(4),
            tag: Some(None),
        },
    )
    .await
    .unwrap();
    assert_eq!(explicit.score, 4);
    assert_eq!(
        explicit.tag, None,
        "Some(None) inserts NULL instead of the default"
    );
    assert_eq!(explicit.note.as_deref(), Some("n"));
    assert_eq!(explicit.doubled, Some(8));
}

/// Applies every combination of unchanged / set / set-NULL over the four
/// updatable columns and checks the stored row field by field. A placeholder
/// or bind-order mistake surfaces as a value landing in the wrong column or
/// as a type error from PostgreSQL.
pub(super) async fn every_partial_update_combination_binds_correctly(pool: &PgPool) {
    use db::patch_targets::models::PatchTarget;

    let labels = [None, Some("L".to_owned())];
    let notes = [None, Some(Some("N".to_owned())), Some(None)];
    let scores = [None, Some(7)];
    let tags = [None, Some(Some("T".to_owned())), Some(None)];

    let mut combinations = 0;
    for label in &labels {
        for note in &notes {
            for score in &scores {
                for tag in &tags {
                    let before = PatchTarget::create(pool, base_patch_target())
                        .await
                        .unwrap();
                    let patch = UpdatePatchTarget {
                        label: label.clone(),
                        note: note.clone(),
                        score: *score,
                        tag: tag.clone(),
                    };

                    let mut expected = before.clone();
                    if let Some(value) = &patch.label {
                        expected.label = value.clone();
                    }
                    if let Some(value) = &patch.note {
                        expected.note = value.clone();
                    }
                    if let Some(value) = patch.score {
                        expected.score = value;
                        expected.doubled = Some(value * 2);
                    }
                    if let Some(value) = &patch.tag {
                        expected.tag = value.clone();
                    }

                    let updated = PatchTarget::patch(pool, before.id, patch.clone())
                        .await
                        .unwrap_or_else(|error| panic!("{patch:?} failed: {error}"));
                    assert_eq!(updated, expected, "RETURNING row for {patch:?}");
                    let stored = PatchTarget::by_primary_key(before.id)
                        .first(pool)
                        .await
                        .unwrap();
                    assert_eq!(stored, Some(expected), "stored row for {patch:?}");
                    combinations += 1;
                }
            }
        }
    }
    assert_eq!(combinations, 36);
}

pub(super) async fn empty_and_missing_updates(pool: &PgPool) {
    let row: PatchTarget = PatchTarget::create(pool, base_patch_target())
        .await
        .unwrap();
    let unchanged = PatchTarget::patch(pool, row.id, UpdatePatchTarget::default())
        .await
        .unwrap();
    assert_eq!(unchanged, row, "empty patch returns the current row");

    let missing_id = PatchTargetId::from(i64::MAX);
    let empty_missing = PatchTarget::patch(pool, missing_id, UpdatePatchTarget::default())
        .await
        .unwrap_err();
    assert!(empty_missing.is_not_found(), "{empty_missing}");
    let missing = PatchTarget::patch(
        pool,
        missing_id,
        UpdatePatchTarget {
            score: Some(1),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert!(
        missing.is_not_found(),
        "updating a missing row is NotFound: {missing}"
    );
}

pub(super) async fn destroy_reports_existing_and_missing_rows(pool: &PgPool) {
    use db::patch_targets::models::PatchTarget;

    let row = PatchTarget::create(pool, base_patch_target())
        .await
        .unwrap();
    assert!(
        PatchTarget::by_primary_key(row.id)
            .exists(pool)
            .await
            .unwrap()
    );
    PatchTarget::destroy(pool, row.id)
        .await
        .expect("existing row is deleted");
    assert!(
        PatchTarget::destroy(pool, row.id)
            .await
            .unwrap_err()
            .is_not_found(),
        "second destroy finds nothing"
    );
    assert!(
        !PatchTarget::by_primary_key(row.id)
            .exists(pool)
            .await
            .unwrap()
    );
    assert_eq!(
        PatchTarget::by_primary_key(row.id)
            .first(pool)
            .await
            .unwrap(),
        None
    );
}

pub(super) async fn transactions_stay_with_the_caller(pool: &PgPool) {
    use db::patch_targets::models::PatchTarget;

    let mut tx = pool.begin().await.unwrap();
    let rolled_back = PatchTarget::create(&mut *tx, base_patch_target())
        .await
        .unwrap();
    assert!(
        PatchTarget::by_primary_key(rolled_back.id)
            .exists(&mut *tx)
            .await
            .unwrap()
    );
    tx.rollback().await.unwrap();
    assert!(
        !PatchTarget::by_primary_key(rolled_back.id)
            .exists(pool)
            .await
            .unwrap()
    );

    let mut tx = pool.begin().await.unwrap();
    let committed = PatchTarget::create(&mut *tx, base_patch_target())
        .await
        .unwrap();
    let patch = UpdatePatchTarget {
        label: Some("in tx".to_owned()),
        ..Default::default()
    };
    PatchTarget::patch(&mut *tx, committed.id, patch)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let stored = PatchTarget::by_primary_key(committed.id)
        .first(pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.label, "in tx");
}

pub(super) async fn all_defaulted_insert_uses_default_values(pool: &PgPool) {
    use db::counters::models::Counter;
    use db::counters::models::NewCounter;

    let empty = Counter::create(pool, NewCounter::default()).await.unwrap();
    assert_eq!(empty.hits, 0);
    let set = Counter::create(
        pool,
        NewCounter {
            hits: Some(3),
            created_at: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(set.hits, 3);
    assert!(set.id > empty.id);
}

pub(super) async fn reserved_and_quoted_identifiers_round_trip(pool: &PgPool) {
    use db::reserved_words::models::ReservedWord;
    use db::reserved_words::models::{NewReservedWord, UpdateReservedWord};

    let row = ReservedWord::create(
        pool,
        NewReservedWord {
            from: "origin".to_owned(),
            type_: Some("kind".to_owned()),
            mixed_case: None,
            user: None,
            _1st: Some(true),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        row.mixed_case, 1,
        "quoted defaulted column applies its default"
    );
    assert_eq!(row.type_.as_deref(), Some("kind"));
    assert_eq!(row._1st, Some(true));

    let patch = UpdateReservedWord {
        type_: Some(None),
        user: Some(Some("alice".to_owned())),
        mixed_case: Some(9),
        ..Default::default()
    };
    let updated = ReservedWord::patch(pool, row.select, patch).await.unwrap();
    assert_eq!(updated.type_, None);
    assert_eq!(updated.user.as_deref(), Some("alice"));
    assert_eq!(updated.mixed_case, 9);
    assert_eq!(updated.from, "origin");

    assert_eq!(ReservedWord::count(pool).await.unwrap(), 1);
    assert_eq!(
        ReservedWord::query()
            .order_by(ReservedWord::SELECT)
            .limit(10)
            .all(pool)
            .await
            .unwrap(),
        vec![updated]
    );
    ReservedWord::destroy(pool, row.select).await.unwrap();
}

pub(super) async fn table_without_primary_key(pool: &PgPool) {
    use db::audit_log::models::AuditLog;
    use db::audit_log::models::NewAuditLog;

    for event in ["first", "second"] {
        AuditLog::create(
            pool,
            NewAuditLog {
                event: event.to_owned(),
                recorded_at: None,
            },
        )
        .await
        .unwrap();
    }
    assert_eq!(AuditLog::count(pool).await.unwrap(), 2);
    let mut events: Vec<String> = AuditLog::all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.event)
        .collect();
    events.sort();
    assert_eq!(events, ["first", "second"]);
}

/// The fixture generates with `derive = ["serde"]`: rows read from the
/// database serialize to JSON and back unchanged. Typed ids serialize as
/// their inner value and enum variants as their database label.
pub(super) async fn rows_round_trip_through_json(pool: &PgPool) {
    use db::products::models::Product;
    use db::tickets::models::Ticket;

    let products = Product::all(pool).await.unwrap();
    assert!(!products.is_empty(), "earlier scenarios left products");
    for product in &products {
        let json = serde_json::to_string(product).unwrap();
        assert_eq!(&serde_json::from_str::<Product>(&json).unwrap(), product);
    }
    let value = serde_json::to_value(&products[0]).unwrap();
    assert_eq!(value["id"], serde_json::json!(products[0].id.0));

    let tickets = Ticket::all(pool).await.unwrap();
    let very_high = tickets
        .iter()
        .find(|ticket| ticket.priority == db::types::TicketPriority::VeryHigh)
        .expect("earlier scenarios left a very high ticket");
    let json = serde_json::to_value(very_high).unwrap();
    assert_eq!(json["priority"], "Very High");
    assert_eq!(&serde_json::from_value::<Ticket>(json).unwrap(), very_high);
}

/// A patch deserialized from JSON leaves the columns absent from it alone
/// and sets those given as `null` to `NULL`; serializing skips absent fields.
pub(super) async fn patches_from_json_tell_absent_from_null(pool: &PgPool) {
    let before = PatchTarget::create(pool, base_patch_target())
        .await
        .unwrap();
    assert_eq!(before.note.as_deref(), Some("n0"));

    let untouched: UpdatePatchTarget = serde_json::from_str("{}").unwrap();
    assert!(untouched.is_empty());
    assert_eq!(serde_json::to_string(&untouched).unwrap(), "{}");
    let row = PatchTarget::patch(pool, before.id, untouched)
        .await
        .unwrap();
    assert_eq!(row, before, "an empty patch changes nothing");

    let cleared: UpdatePatchTarget = serde_json::from_str(r#"{"note": null, "score": 9}"#).unwrap();
    assert_eq!(cleared.note, Some(None));
    assert_eq!(cleared.tag, None);
    assert_eq!(
        serde_json::to_value(&cleared).unwrap(),
        serde_json::json!({"note": null, "score": 9})
    );
    let row = PatchTarget::patch(pool, before.id, cleared).await.unwrap();
    assert_eq!(row.note, None, "`null` sets the column to NULL");
    assert_eq!(row.score, 9);
    assert_eq!(row.tag, before.tag, "an absent column is left alone");
    assert_eq!(row.label, before.label);

    let set: UpdatePatchTarget = serde_json::from_str(r#"{"note": "again"}"#).unwrap();
    assert_eq!(set.note, Some(Some("again".to_owned())));
}
