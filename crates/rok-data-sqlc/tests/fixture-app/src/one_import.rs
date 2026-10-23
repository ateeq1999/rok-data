//! One import is enough (dx-first ticket 02): a module other than the one
//! declaring the models reaches reads, row writes, and chained scopes with
//! `use crate::db::prelude::*;` and nothing else. Compile-only — the
//! `fixture_compile` test runs `cargo clippy` over the crate.

#![allow(dead_code)]

use crate::db::prelude::*;

async fn reads_writes_and_scopes(pool: &sqlx::PgPool) -> Result<(), rok_data::Error> {
    // A read verb on the model.
    let found = Supplier::find(pool, uuid::Uuid::nil()).await?;

    // A row write.
    if let Some(mut supplier) = found {
        supplier.name = "Renamed".into();
        supplier.update(pool).await?;
    }

    // A scope entry, then the same scope chained.
    let _ = Supplier::active().active().all(pool).await?;
    Ok(())
}
