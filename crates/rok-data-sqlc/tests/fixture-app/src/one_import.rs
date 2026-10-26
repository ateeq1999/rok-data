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

/// The generated write verbs, as `docs/getting-started.md` quotes them.
async fn generated_verbs(db: rok_data::Db) -> Result<(), rok_data::Error> {
    // generated models also carry their write verbs, taking the generated
    // inputs: `create`, `patch`, `destroy` (and `restore` /
    // `delete_permanently` with soft delete)
    let supplier = Supplier::create(
        &db,
        NewSupplier {
            id: SupplierId::from(uuid::Uuid::now_v7()),
            name: "Acme".into(),
            contact_email: None,
            is_active: None,
            created_at: None,
        },
    )
    .await?;
    let patch = UpdateSupplier {
        name: Some("Acme Ltd".into()),
        ..UpdateSupplier::default()
    };
    Supplier::patch(&db, supplier.id, patch).await?;
    Supplier::destroy(&db, supplier.id).await?;
    Ok(())
}

/// A handler's error, for the not-found sample of `docs/migration-guide.md`.
#[derive(Debug)]
enum AppError {
    NotFound,
    Data(rok_data::Error),
}

impl From<rok_data::Error> for AppError {
    fn from(error: rok_data::Error) -> Self {
        AppError::Data(error)
    }
}

fn not_found() -> Result<User, AppError> {
    Err(AppError::NotFound)
}

/// The one "not found" convention, as `docs/migration-guide.md` quotes it.
async fn not_found_convention(
    db: rok_data::Db,
    id: UserId,
    patch: UpdateUser,
) -> Result<User, AppError> {
    // after
    let user = match User::patch(&db, id, patch).await {
        Ok(user) => user,
        Err(error) if error.is_not_found() => return not_found(),
        Err(error) => return Err(error.into()),
    };
    User::destroy(&db, id).await?; // `?` carries NotFound to your error type
    Ok(user)
}
