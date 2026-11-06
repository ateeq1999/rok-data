//! Annotated query files.

use super::*;

pub(super) async fn annotated_queries(pool: &PgPool) {
    use db::products::models::ProductId;
    use db::queries::inventory::{self as queries, CreateProductsParams};

    let category = queries::create_category(pool, "query-cat", None)
        .await
        .unwrap();
    assert_eq!(category.name, "query-cat");
    assert_eq!(category.description, None);

    let supplier = db::suppliers::models::Supplier::create(
        pool,
        db::suppliers::models::NewSupplier {
            id: uuid::Uuid::from_u128(900).into(),
            name: "Acme".to_owned(),
            contact_email: None,
            is_active: None,
            created_at: None,
        },
    )
    .await
    .unwrap();

    let inserted = queries::create_products(
        pool,
        CreateProductsParams {
            id: ProductId::from(uuid::Uuid::from_u128(101)),
            sku: "Q-1".to_owned(),
            name: "Query hammer".to_owned(),
            category_id: category.id,
            supplier_id: Some(supplier.id),
            id_2: ProductId::from(uuid::Uuid::from_u128(102)),
            sku_2: "Q-2".to_owned(),
            name_2: "Query anvil".to_owned(),
            category_id_2: category.id,
            supplier_id_2: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        inserted, 2,
        "both VALUES rows bind to the right placeholders"
    );

    let hammer = queries::get_product_by_sku(pool, "Q-1")
        .await
        .unwrap()
        .expect("Q-1 exists");
    assert_eq!(hammer.id, ProductId::from(uuid::Uuid::from_u128(101)));
    assert_eq!(hammer.name, "Query hammer");
    assert_eq!(
        queries::get_product_by_sku(pool, "missing").await.unwrap(),
        None
    );

    queries::update_product_stock(pool, ProductId::from(uuid::Uuid::from_u128(101)), 50)
        .await
        .unwrap();
    let low_stock: Vec<String> = queries::list_products_by_category(pool, category.id, 10)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.sku)
        .collect();
    assert_eq!(low_stock, ["Q-2"], "only the anvil has stock <= 10");

    let joined = queries::list_products_with_category(pool, &["Q-1".to_owned(), "Q-2".to_owned()])
        .await
        .unwrap();
    assert_eq!(joined.len(), 2);
    assert_eq!(joined[0].category_name, "query-cat");
    assert_eq!(joined[0].supplier_name.as_deref(), Some("Acme"));
    assert_eq!(
        joined[1].supplier_name, None,
        "LEFT JOIN without a supplier"
    );

    assert_eq!(
        queries::count_products_in_category(pool, category.id)
            .await
            .unwrap(),
        2
    );
    let stock = queries::category_stock(pool).await.unwrap();
    let ours = stock
        .iter()
        .find(|row| row.category_id == category.id)
        .expect("category has stock");
    assert_eq!(ours.total_stock, 50);

    let stocked = queries::stocked_categories(pool, 1, category.id)
        .await
        .unwrap();
    assert_eq!(stocked.len(), 1, "the CTE pre-aggregates per category");
    assert_eq!(stocked[0].id, category.id);
    assert_eq!(stocked[0].category_name, "query-cat");
    assert_eq!(
        stocked[0].product_count, 1,
        "only the hammer has stock >= 1"
    );
    assert_eq!(stocked[0].total_stock, Some(50));
    assert!(
        queries::stocked_categories(pool, 51, category.id)
            .await
            .unwrap()
            .is_empty(),
        "the CTE placeholder filters before aggregation"
    );

    let found = queries::search_products(pool, "%ANVIL%").await.unwrap();
    assert_eq!(found, ["Q-2"]);

    assert_eq!(
        queries::delete_product_by_sku(pool, "Q-2").await.unwrap(),
        1
    );
    assert_eq!(
        queries::delete_product_by_sku(pool, "Q-2").await.unwrap(),
        0
    );
}

/// Each query file is its own module, so both define `search_products`.
/// Runs after `annotated_queries`, which leaves product Q-1 supplied by Acme.
pub(super) async fn query_modules_per_file(pool: &PgPool) {
    use db::queries::{inventory, suppliers};

    let globex_id = db::suppliers::models::SupplierId::from(uuid::Uuid::from_u128(901));
    let globex = suppliers::create_supplier(pool, globex_id, "Globex")
        .await
        .unwrap();
    assert_eq!(globex.name, "Globex");
    assert!(globex.is_active, "the column default applies");

    assert_eq!(
        suppliers::search_products(pool, "Acme").await.unwrap(),
        ["Q-1"]
    );
    assert!(
        suppliers::search_products(pool, "Globex")
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        inventory::search_products(pool, "%hammer%").await.unwrap(),
        ["Q-1"]
    );
}

/// `hierarchy.sql`: an upsert targeting the partial unique index over live
/// `soft_notes` rows, and a recursive walk down `org_units`.
pub(super) async fn partial_index_upserts_and_recursive_ctes(pool: &PgPool) {
    use db::org_units::models::NewOrgUnit;
    use db::queries::hierarchy::{org_subtree, upsert_live_soft_note};

    let first = upsert_live_soft_note(pool, "UPSERT", "first")
        .await
        .unwrap();
    let second = upsert_live_soft_note(pool, "UPSERT", "second")
        .await
        .unwrap();
    assert_eq!(second.id, first.id, "the live row is updated in place");
    assert_eq!(second.body, "second");
    // A soft-deleted row is outside the index, so the code is free again.
    db::soft_notes::models::SoftNote::destroy(pool, first.id)
        .await
        .unwrap();
    let third = upsert_live_soft_note(pool, "UPSERT", "third")
        .await
        .unwrap();
    assert_ne!(third.id, first.id);
    // Later scenarios count every `soft_notes` row.
    for id in [first.id, third.id] {
        db::soft_notes::models::SoftNote::delete_permanently(pool, id)
            .await
            .unwrap();
    }

    let unit = async |parent_id, name: &str| {
        db::org_units::models::OrgUnit::create(
            pool,
            NewOrgUnit {
                parent_id,
                name: name.to_owned(),
            },
        )
        .await
        .unwrap()
    };
    let root = unit(None, "tree root").await;
    let left = unit(Some(root.id), "tree left").await;
    let right = unit(Some(root.id), "tree right").await;
    let leaf = unit(Some(left.id), "tree leaf").await;
    unit(None, "other root").await;

    let walk: Vec<_> = org_subtree(pool, root.id)
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.id, row.name, row.depth))
        .collect();
    assert_eq!(
        walk,
        [
            (root.id, root.name, 0),
            (left.id, left.name, 1),
            (right.id, right.name, 1),
            (leaf.id, leaf.name, 2),
        ]
    );
}

/// A `:page` query (Karibu Pass's partner bookings: a date range and a
/// search across four columns) returns one page of its rows, in its order,
/// with the total across every page; past the last page the page is empty
/// and still reports the total.
pub(super) async fn page_queries_return_rows_and_the_total(pool: &PgPool) {
    use chrono::NaiveDate;
    use db::queries::bookings::{PartnerBookingsParams, partner_bookings};

    sqlx::raw_sql(
        "INSERT INTO bookings (partner_id, reference, guest_name, guest_email, offer_title, starts_on) VALUES
            (1, 'KB-001', 'Amina Otieno', 'amina@example.com', 'Lamu dhow trip', '2026-03-01'),
            (1, 'KB-002', 'Brian Kip', 'brian@example.com', 'Safari lodge', '2026-03-05'),
            (1, 'KB-003', 'Chao Li', 'chao@lamu.example', 'City walk', '2026-03-09'),
            (1, 'LAMU-4', 'Dana Rose', 'dana@example.com', 'Beach day', '2026-03-12'),
            (1, 'KB-005', 'Lamuel Grey', 'lg@example.com', 'Night market', '2026-03-20'),
            (1, 'KB-006', 'Erin Moss', 'erin@example.com', 'Lamu food tour', '2026-04-02'),
            (2, 'KB-007', 'Fay Lamu', 'fay@example.com', 'Lamu dhow trip', '2026-03-03')",
    )
    .execute(pool)
    .await
    .unwrap();

    let march = |page, per_page| {
        partner_bookings(
            pool,
            PartnerBookingsParams {
                partner_id: 1,
                starts_on_from: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
                starts_on_to: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
                search: "%lamu%".to_owned(),
            },
            page,
            per_page,
        )
    };
    let references = |page: &rok_data::Page<_>| -> Vec<String> {
        page.items
            .iter()
            .map(|row: &db::queries::bookings::PartnerBookingsRow| row.reference.clone())
            .collect()
    };

    // Each of the four columns matches once in March: the offer title
    // (KB-001), the guest email (KB-003), the reference (LAMU-4) and the
    // guest name (KB-005). April's and partner 2's don't count.
    let first = march(1, 3).await.unwrap();
    assert_eq!(references(&first), ["KB-005", "LAMU-4", "KB-003"]);
    assert_eq!((first.total, first.page, first.per_page), (4, 1, 3));

    let second = march(2, 3).await.unwrap();
    assert_eq!(references(&second), ["KB-001"]);
    assert_eq!(second.total, 4);

    let past = march(5, 3).await.unwrap();
    assert!(past.items.is_empty());
    assert_eq!((past.total, past.page), (4, 5));

    // A caller's transaction runs both statements inside it.
    let mut tx = pool.begin().await.unwrap();
    let inside = partner_bookings(
        &mut *tx,
        PartnerBookingsParams {
            partner_id: 2,
            starts_on_from: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            starts_on_to: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
            search: "%LAMU%".to_owned(),
        },
        1,
        10,
    )
    .await
    .unwrap();
    assert_eq!((references(&inside), inside.total), (vec!["KB-007".to_owned()], 1));
    tx.rollback().await.unwrap();

    assert!(matches!(
        march(1, 0).await,
        Err(rok_data::Error::InvalidQuery(_))
    ));
}
