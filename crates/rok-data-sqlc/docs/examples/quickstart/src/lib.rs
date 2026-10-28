//! The README's quickstart. `rok sqlc generate` generates `src/db` from
//! `migrations/` and `queries.sql`, as `rok-data-sqlc.toml` configures.

#![deny(missing_docs)]

pub mod db;

use db::categories::models::{Category, NewCategory};
use db::products::filter::{ProductFilter, ProductSortKey};
use db::products::models::{NewProduct, Product, UpdateProduct};
use db::search::{Ordered, Sort, Text};
use db::types::ProductStatus;
use rok_data::Model;
use rust_decimal::Decimal;
use sqlx::PgPool;

/// Creates a category and a product, then reads them back.
///
/// # Errors
///
/// Any database error.
pub async fn quickstart(pool: &PgPool) -> Result<(), rok_data::Error> {
    // Every generated function takes an executor first: here, the pool.
    let tools = Category::create(
        pool,
        NewCategory {
            name: "Tools".into(),
            created_at: None,
        },
    )
    .await?;

    // Several calls in one transaction: pass `&mut *tx` instead.
    let mut tx = pool.begin().await?;
    let hammer = Product::create(
        &mut *tx,
        NewProduct {
            id: None,
            category_id: tools.id,
            sku: "HAM-1".into(),
            name: "Claw hammer".into(),
            price: Decimal::new(1999, 2),
            status: Some(ProductStatus::Active),
            tags: None,
            created_at: None,
            description: None,
        },
    )
    .await?;
    let patch = UpdateProduct {
        price: Some(Decimal::new(1799, 2)),
        description: Some(Some("16 oz, fibreglass handle".into())),
        ..UpdateProduct::default()
    };
    Product::patch(&mut *tx, hammer.id, patch).await?;
    tx.commit().await?;

    // Find by the typed id, or read through a unique key's chain.
    let by_id = Product::find(pool, hammer.id).await?;
    let by_sku = Product::by_sku("HAM-1").first(pool).await?;
    assert_eq!(by_id, by_sku);

    // Typed search: a filter, then sort and page on the chain.
    let filter = ProductFilter {
        price: vec![Ordered::Lt(Decimal::new(5000, 2))],
        name: vec![Text::Contains("hammer".into())],
        ..ProductFilter::default()
    };
    let cheap = Product::search(filter)?
        .order_by(Sort::asc(ProductSortKey::Price))
        .limit(20)
        .all(pool)
        .await?;

    // Relationships: every product's category, in one query.
    let categories = db::products::relationships::load_category(pool, &cheap).await?;
    for product in &cheap {
        let category = &categories[&product.category_id];
        println!("{} ({})", product.name, category.name);
    }

    // Query-file functions live in `db::queries`.
    for row in db::queries::product_counts_by_category(pool).await? {
        println!("{}: {}", row.name, row.product_count);
    }
    Ok(())
}
