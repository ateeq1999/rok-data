//! The generated `db/schema.rs`: its `SCHEMA_SQL` is what a project's
//! tests load into an empty database, so it runs as written there.

use super::*;

/// `users` (file 0001) references `cities` (file 0007): file-name order
/// creates `users` first.
const USERS: &str = "CREATE TABLE users (\n\
     \x20   id BIGSERIAL PRIMARY KEY,\n\
     \x20   email TEXT NOT NULL,\n\
     \x20   home_city_id BIGINT REFERENCES cities (id)\n\
     );\n\
     CREATE INDEX users_home_city_id_idx ON users (home_city_id);\n";
const CITIES: &str = "CREATE TABLE cities (\n\
     \x20   id BIGSERIAL PRIMARY KEY,\n\
     \x20   name TEXT NOT NULL\n\
     );\n";

/// The text of the generated `SCHEMA_SQL` raw string.
fn schema_sql(schema_rs: &str) -> String {
    let (_, rest) = schema_rs
        .split_once("pub const SCHEMA_SQL: &str = r")
        .expect("schema.rs declares SCHEMA_SQL");
    let hashes: String = rest.chars().take_while(|c| *c == '#').collect();
    let rest = &rest[hashes.len() + 1..];
    let end = rest
        .find(&format!("\"{hashes};"))
        .expect("SCHEMA_SQL's raw string ends");
    rest[..end].to_owned()
}

#[test]
fn schema_sql_with_a_forward_reference_across_files_loads() -> Result<()> {
    let dir = TempDir::new()?;
    let schema = dir.path().join("schema");
    fs::create_dir(&schema)?;
    fs::write(schema.join("0001_users.sql"), USERS)?;
    fs::write(schema.join("0007_cities.sql"), CITIES)?;
    fs::write(
        dir.path().join("rok-data-sqlc.toml"),
        "schema = [\"schema\"]\noutput = \"out\"\n",
    )?;
    assert_success(&run_binary_in(dir.path(), &["g"]));
    let sql = schema_sql(&fs::read_to_string(dir.path().join("out/schema.rs"))?);
    // Each statement is kept as written.
    for statement in [USERS.split_once(";\n").unwrap().0, CITIES.trim_end()] {
        assert!(sql.contains(statement), "{statement:?} not in:\n{sql}");
    }

    let Ok(url) = std::env::var("ROK_DATA_SQLC_TEST_DATABASE_URL") else {
        eprintln!("skipping the load: ROK_DATA_SQLC_TEST_DATABASE_URL is not set");
        return Ok(());
    };
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let db_schema = format!("rok_snapshot_{}_{nanos}", std::process::id());
    let separator = if url.contains('?') { '&' } else { '?' };
    let scoped = format!("{url}{separator}options=-c%20search_path%3D{db_schema}");
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let admin = rok_data::Db::connect(&url).await?;
            admin.execute(&format!("CREATE SCHEMA {db_schema}")).await?;
            let loaded = async {
                let db = rok_data::Db::connect(&scoped).await?;
                db.execute(&sql).await?;
                anyhow::Ok(())
            }
            .await;
            admin
                .execute(&format!("DROP SCHEMA {db_schema} CASCADE"))
                .await?;
            loaded.map_err(|err| anyhow::anyhow!("SCHEMA_SQL did not load: {err}\n{sql}"))
        })
}
