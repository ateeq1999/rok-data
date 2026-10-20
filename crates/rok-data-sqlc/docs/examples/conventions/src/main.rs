//! Runs the conventions example against a database the migrations have been
//! applied to: `DATABASE_URL=postgres://localhost/invoices cargo run`.

#[tokio::main]
async fn main() -> Result<(), rok_data::Error> {
    let url = std::env::var("DATABASE_URL").expect("set DATABASE_URL");
    let pool = sqlx::PgPool::connect(&url).await?;
    rok_data_sqlc_conventions::invoices(&pool, uuid::Uuid::nil()).await
}
