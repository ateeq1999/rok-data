//! Parameter and column types of queries, as PostgreSQL reports them, and
//! the database word on the schema (workspace ADR 0010): the parse runs offline first,
//! and a database at hand cross-checks it.
//!
//! Queries are prepared in a temporary database that has every migration
//! applied. Results are cached in `queries.json` (committed next to the
//! generated code), so later runs and CI need no database until a query or
//! the schema changes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::{
    AssertSqlSafe, Column as _, Connection, Executor as _, PgConnection, SqlSafeStr as _,
    TypeInfo as _,
};

use crate::Error;
use crate::ir::{Query, Schema};

/// What PostgreSQL says about one query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Description {
    /// Parameter type names (`INT8`, `TEXT[]`, `user_role`).
    pub(crate) params: Vec<String>,
    /// Result columns.
    pub(crate) columns: Vec<DescribedColumn>,
}

/// A result column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DescribedColumn {
    pub(crate) name: String,
    pub(crate) type_name: String,
    /// `Some(false)` when PostgreSQL knows it is never NULL.
    pub(crate) nullable: Option<bool>,
}

/// The cache file.
#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Cache {
    /// Keyed by a hash of the schema and the query text.
    pub(crate) queries: BTreeMap<String, CachedQuery>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CachedQuery {
    /// Where the query is, to make the file readable.
    pub(crate) query: String,
    #[serde(flatten)]
    pub(crate) description: Description,
}

pub(crate) fn key(migrations_sql: &str, query_sql: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in migrations_sql.bytes().chain([0]).chain(query_sql.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Describe `queries`, using `cache` where possible. Returns the
/// descriptions and the new cache (only entries still in use).
pub(crate) fn describe_all(
    queries: &[Query],
    migrations_sql: &[String],
    cache: &Cache,
    database_url: Option<&str>,
) -> Result<(Vec<Description>, Cache), Error> {
    let schema_text = migrations_sql.join("\n");
    let keys: Vec<String> = queries.iter().map(|q| key(&schema_text, &q.sql)).collect();
    let missing: Vec<usize> = (0..queries.len())
        .filter(|&i| !cache.queries.contains_key(&keys[i]))
        .collect();
    let mut fresh: BTreeMap<usize, Description> = BTreeMap::new();
    if !missing.is_empty() {
        let Some(url) = database_url else {
            let names: Vec<String> = missing
                .iter()
                .map(|&i| format!("`{}` ({})", queries[i].name, queries[i].location))
                .collect();
            return Err(Error::new(format!(
                "these queries are new or the schema changed, so PostgreSQL has to check them: {}.\n\
                 Set DATABASE_URL to a server where the user may create databases; rok \
                 creates a temporary database, applies the migrations, prepares the queries and \
                 drops it. The results are saved in queries.json for later runs.",
                names.join(", ")
            )));
        };
        let to_check: Vec<&Query> = missing.iter().map(|&i| &queries[i]).collect();
        // A thread of its own, so this also works when called from async code.
        let described = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| Error::new(format!("can't start an async runtime: {e}")))?
                        .block_on(describe_in_temp_db(url, migrations_sql, &to_check))
                })
                .join()
                .unwrap_or_else(|_| Err(Error::new("checking the queries panicked")))
        })?;
        fresh = missing.into_iter().zip(described).collect();
    }
    let mut out = Cache::default();
    let mut descriptions = Vec::new();
    for (i, query) in queries.iter().enumerate() {
        let description = match fresh.remove(&i) {
            Some(d) => d,
            None => cache.queries[&keys[i]].description.clone(),
        };
        out.queries.insert(
            keys[i].clone(),
            CachedQuery {
                query: format!("{}::{}", query.module, query.name),
                description: description.clone(),
            },
        );
        descriptions.push(description);
    }
    Ok((descriptions, out))
}

async fn describe_in_temp_db(
    url: &str,
    migrations_sql: &[String],
    queries: &[&Query],
) -> Result<Vec<Description>, Error> {
    use std::str::FromStr;
    let db_error = |what: &str, e: sqlx::Error| Error::new(format!("{what}: {e}"));
    let admin_options = sqlx::postgres::PgConnectOptions::from_str(url)
        .map_err(|e| db_error("DATABASE_URL is not a valid PostgreSQL URL", e))?;
    let mut admin = PgConnection::connect_with(&admin_options)
        .await
        .map_err(|e| db_error("can't connect to DATABASE_URL", e))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let name = format!("rok_gen_{}_{nanos}", std::process::id());
    // `name` is generated above, never input; the scripts below are the
    // project's own migrations.
    sqlx::raw_sql(AssertSqlSafe(format!(r#"CREATE DATABASE "{name}""#)))
        .execute(&mut admin)
        .await
        .map_err(|e| {
            db_error(
                "can't create a temporary database (the user needs CREATEDB)",
                e,
            )
        })?;
    let result = async {
        let mut conn = PgConnection::connect_with(&admin_options.clone().database(&name))
            .await
            .map_err(|e| db_error("can't connect to the temporary database", e))?;
        for (i, sql) in migrations_sql.iter().enumerate() {
            sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .execute(&mut conn)
                .await
                .map_err(|e| {
                    db_error(
                        &format!("migration {} failed in the temporary database", i + 1),
                        e,
                    )
                })?;
        }
        let mut out = Vec::new();
        for query in queries {
            let described = conn
                .describe(AssertSqlSafe(query.sql.as_str()).into_sql_str())
                .await
                .map_err(|e| {
                    Error::at(
                        &query.location,
                        format!("PostgreSQL rejected `{}`: {e}", query.name),
                    )
                })?;
            let params = match described.parameters() {
                Some(sqlx::Either::Left(types)) => {
                    types.iter().map(|t| t.name().to_owned()).collect()
                }
                Some(sqlx::Either::Right(0)) | None => Vec::new(),
                Some(sqlx::Either::Right(_)) => {
                    return Err(Error::at(
                        &query.location,
                        "PostgreSQL didn't report the parameter types",
                    ));
                }
            };
            let columns = described
                .columns()
                .iter()
                .enumerate()
                .map(|(i, c)| DescribedColumn {
                    name: c.name().to_owned(),
                    type_name: c.type_info().name().to_owned(),
                    nullable: described.nullable(i),
                })
                .collect();
            out.push(Description { params, columns });
        }
        conn.close().await.ok();
        Ok(out)
    }
    .await;
    // Drop the temporary database even when something failed.
    let dropped = sqlx::raw_sql(AssertSqlSafe(format!(
        r#"DROP DATABASE IF EXISTS "{name}" WITH (FORCE)"#
    )))
    .execute(&mut admin)
    .await;
    let out = result?;
    dropped.map_err(|e| db_error("can't drop the temporary database", e))?;
    Ok(out)
}

/// What a database built from the migrations disagrees with `schema` on
/// (workspace ADR 0010): a warning per difference, empty when the parse and the
/// catalog agree. `--strict` escalates the list to an error, in
/// `generate` — that separation lives there, not here.
pub(crate) fn verify_schema(
    schema: &Schema,
    up_scripts: &[String],
    database_url: &str,
) -> Result<Vec<String>, Error> {
    // A thread of its own, so this also works when called from async code.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| Error::new(format!("can't start an async runtime: {e}")))?
                    .block_on(check_in_temp_db(schema, up_scripts, database_url))
            })
            .join()
            .unwrap_or_else(|_| Err(Error::new("checking the schema panicked")))
    })
}

async fn check_in_temp_db(
    schema: &Schema,
    up_scripts: &[String],
    url: &str,
) -> Result<Vec<String>, Error> {
    use std::str::FromStr;
    let db_error = |what: &str, e: sqlx::Error| Error::new(format!("{what}: {e}"));
    let admin_options = sqlx::postgres::PgConnectOptions::from_str(url)
        .map_err(|e| db_error("DATABASE_URL is not a valid PostgreSQL URL", e))?;
    let mut admin = PgConnection::connect_with(&admin_options)
        .await
        .map_err(|e| db_error("can't connect to DATABASE_URL", e))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let name = format!("rok_verify_{}_{nanos}", std::process::id());
    // `name` is generated above, never input; the scripts below are the
    // project's own migrations.
    sqlx::raw_sql(AssertSqlSafe(format!(r#"CREATE DATABASE "{name}""#)))
        .execute(&mut admin)
        .await
        .map_err(|e| {
            db_error(
                "can't create a temporary database (the user needs CREATEDB)",
                e,
            )
        })?;
    let checked = async {
        let pool = sqlx::postgres::PgPool::connect_with(admin_options.clone().database(&name))
            .await
            .map_err(|e| db_error("can't connect to the temporary database", e))?;
        let checked = async {
            for (i, sql) in up_scripts.iter().enumerate() {
                sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .execute(&pool)
                    .await
                    .map_err(|e| {
                        db_error(
                            &format!("migration {} failed in the temporary database", i + 1),
                            e,
                        )
                    })?;
            }
            database_drift(schema, &pool).await
        }
        .await;
        pool.close().await;
        checked
    }
    .await;
    // Drop the temporary database even when something failed.
    let dropped = sqlx::raw_sql(AssertSqlSafe(format!(
        r#"DROP DATABASE IF EXISTS "{name}" WITH (FORCE)"#
    )))
    .execute(&mut admin)
    .await;
    let drift = checked?;
    dropped.map_err(|e| db_error("can't drop the temporary database", e))?;
    Ok(drift)
}

/// The differences between the parsed schema and the catalog of `pool`, in
/// schema order. The catalog is reached through rok-data's own
/// introspection, the same API `rok db inspect` reads, so generation never
/// grows a second reader of PostgreSQL internals.
async fn database_drift(schema: &Schema, pool: &sqlx::PgPool) -> Result<Vec<String>, Error> {
    use rok_data::introspect::{self, ColumnInfo, ForeignKeyInfo, IndexInfo};
    let db_read = |e: rok_data::Error, what: String| Error::new(format!("{what}: {e}"));
    let db = rok_data::Db::from_pool(pool.clone());

    let mut drift = Vec::new();
    let live_tables = introspect::tables(&db).await.map_err(|e| {
        db_read(
            e,
            "reading the user tables of the temporary database".to_owned(),
        )
    })?;
    let live_by_name: std::collections::BTreeMap<String, ()> =
        live_tables.iter().map(|name| (name.clone(), ())).collect();

    let mut checked: Vec<String> = Vec::new();
    for table in schema.tables.iter().filter(|t| !t.name.contains('.')) {
        // The introspection covers the `public` schema; declaration-order
        // tables have no aliases in the catalog at this point.
        checked.push(table.name.clone());
        if !live_by_name.contains_key(&table.name) {
            drift.push(format!(
                "the database doesn't have the `{}` table",
                table.name
            ));
            continue;
        }
        let live_columns: std::collections::BTreeMap<String, ColumnInfo> =
            introspect::columns(&db, &table.name)
                .await
                .map_err(|e| db_read(e, format!("reading the columns of `{}`", table.name)))?
                .into_iter()
                .map(|info| (info.name.clone(), info))
                .collect();

        for column in &table.columns {
            let Some(info) = live_columns.get(&column.name) else {
                drift.push(format!(
                    "the database doesn't have the `{}` column of `{}`",
                    column.name, table.name
                ));
                continue;
            };
            let live_type = canonical_catalog_type(&info.data_type);
            if !same_type(&column.sql_type, &live_type) {
                drift.push(format!(
                    "the `{}` column of `{}` should be `{}` but the database has `{}`",
                    column.name, table.name, column.sql_type, live_type
                ));
            }
            if column.nullable != info.nullable {
                drift.push(if column.nullable {
                    format!(
                        "the `{}` column of `{}` should allow NULL but the database forbids it",
                        column.name, table.name
                    )
                } else {
                    format!(
                        "the `{}` column of `{}` should be NOT NULL but the database allows NULL",
                        column.name, table.name
                    )
                });
            }
            if table.primary_key.contains(&column.name) != info.is_pk {
                drift.push(if info.is_pk {
                    format!(
                        "the `{}` column of `{}` is in the primary key, but the migrations don't put it there",
                        column.name, table.name
                    )
                } else {
                    format!(
                        "the `{}` column of `{}` should be in the primary key, but the database keeps it outside",
                        column.name, table.name
                    )
                });
            }
            // `DEFAULT` expressions are not compared: the catalog spins
            // casts onto them (`'draft'::status`), which the parser never
            // sees.
        }
        for name in live_columns.keys() {
            if !table.columns.iter().any(|c| &c.name == name) {
                drift.push(format!(
                    "the database has a `{name}` column on `{table_name}`, but the migrations don't declare it",
                    table_name = table.name
                ));
            }
        }

        let live_indexes: std::collections::BTreeMap<String, IndexInfo> =
            introspect::indexes(&db, &table.name)
                .await
                .map_err(|e| db_read(e, format!("reading the indexes of `{}`", table.name)))?
                .into_iter()
                .map(|info| (info.name.clone(), info))
                .collect();
        for unique in &table.uniques {
            let Some(index) = live_indexes.get(&unique.name) else {
                drift.push(format!(
                    "the database doesn't have the `{}` unique key on ({})",
                    unique.name,
                    unique.columns.join(", ")
                ));
                continue;
            };
            if !index.unique || index.columns != unique.columns {
                drift.push(format!(
                    "the `{}` unique key should cover ({}) but the database indexes ({})",
                    unique.name,
                    unique.columns.join(", "),
                    index.columns.join(", ")
                ));
            }
        }
        // The parser holds every index a migration creates: unique keys as
        // constraints, and `CREATE INDEX` statements by name (their
        // definitions are free SQL, re-applied verbatim).
        for index in schema.indexes.iter().filter(|ix| ix.table == table.name) {
            if !live_indexes.contains_key(&index.name) {
                drift.push(format!(
                    "the database doesn't have the `{}` index on `{}`",
                    index.name, index.table
                ));
            }
        }
        for (name, info) in &live_indexes {
            // The primary key's backing index is not a declared key.
            if info.primary {
                continue;
            }
            let declared = table.uniques.iter().any(|u| &u.name == name)
                || schema
                    .indexes
                    .iter()
                    .any(|ix| &ix.name == name && ix.table == table.name);
            if !declared {
                drift.push(format!(
                    "the database has an index `{name}` on `{}`, which the migrations don't declare",
                    table.name
                ));
            }
        }

        let live_fks: std::collections::BTreeMap<String, ForeignKeyInfo> =
            introspect::foreign_keys(&db, &table.name)
                .await
                .map_err(|e| db_read(e, format!("reading the foreign keys of `{}`", table.name)))?
                .into_iter()
                .map(|info| (info.constraint_name.clone(), info))
                .collect();
        for key in &table.foreign_keys {
            let Some(info) = live_fks.get(&key.name) else {
                drift.push(format!(
                    "the database doesn't have the `{}` reference from `{}`",
                    key.name, table.name
                ));
                continue;
            };
            let cataloged = leaf(&key.ref_table);
            if info.columns != key.columns
                || info.foreign_table != cataloged
                || info.foreign_columns != key.ref_columns
            {
                drift.push(format!(
                    "the `{name}` reference should go from ({columns}) to `{to}` ({to_columns}), but the database wires it from ({from_columns}) to `{from_table}` ({from_reference_columns})",
                    name = key.name,
                    columns = key.columns.join(", "),
                    to = cataloged,
                    to_columns = key.ref_columns.join(", "),
                    from_columns = info.columns.join(", "),
                    from_table = info.foreign_table,
                    from_reference_columns = info.foreign_columns.join(", "),
                ));
            }
        }
        for (name, info) in &live_fks {
            if !table.foreign_keys.iter().any(|k| &k.name == name) {
                drift.push(format!(
                    "the database has a `{name}` reference from ({}) to {}.{} ({}), which the migrations don't declare",
                    info.columns.join(", "),
                    info.foreign_table,
                    info.foreign_columns.join(", "),
                    info.constraint_name,
                ));
            }
        }
    }
    for name in &live_tables {
        if !checked.contains(name) {
            drift.push(format!(
                "the database has a `{name}` table, but the migrations don't create it"
            ));
        }
    }
    Ok(drift)
}

/// The catalog's type name, in the parser's canonical spelling. The catalog
/// names arrays with a leading underscore and knows no enum spellings
/// beyond their names.
fn canonical_catalog_type(data_type: &str) -> String {
    match data_type.strip_prefix('_') {
        Some(base) => format!("{}[]", crate::types::canonical(base)),
        None => crate::types::canonical(data_type),
    }
}

/// Whether the parser's canonical type and the catalog's report the same
/// type. The catalog knows a serial type only as its base integer type, and
/// reports `NUMERIC` without its precision (which `sqlx` never sees).
fn same_type(parser_type: &str, catalog_type: &str) -> bool {
    let parser = crate::types::canonical(parser_type);
    let parser = match parser.as_str() {
        "BIGSERIAL" => "BIGINT",
        "SMALLSERIAL" => "SMALLINT",
        "SERIAL" => "INTEGER",
        other => other,
    };
    if parser == catalog_type {
        return true;
    }
    base_of(parser) == base_of(catalog_type) && base_of(parser) == "NUMERIC"
}

fn base_of(name: &str) -> &str {
    match name.split_once('(') {
        Some((base, _)) => base,
        None => name,
    }
}

/// The name's leaf, without a schema qualification.
fn leaf(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{SourceFile, parse};

    fn schema_of(sql: &str) -> Schema {
        let sources = [SourceFile {
            path: "db/app.sql".to_owned(),
            module: "app".to_owned(),
            text: sql.to_owned(),
        }];
        parse(&sources).expect("parses").schema
    }

    #[test]
    fn verify_agrees_with_the_worked_schema_and_names_the_drift() {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            println!("skip: verify_schema needs DATABASE_URL");
            return;
        };
        let full = "
CREATE TYPE user_status AS ENUM ('draft', 'live');
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    tags TEXT[] NOT NULL,
    price NUMERIC(12, 2),
    status user_status NOT NULL DEFAULT 'draft'
);
CREATE UNIQUE INDEX users_tags_idx ON users (tags);
";
        // A database built without the `tags` column (its index follows it):
        // both differences are reported, and only those.
        let applied = "
CREATE TYPE user_status AS ENUM ('draft', 'live');
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    price NUMERIC(12, 2),
    status user_status NOT NULL DEFAULT 'draft'
);
";
        let schema = schema_of(full);

        let drift = verify_schema(&schema, &[applied.to_owned()], &url).expect("the check runs");
        assert_eq!(drift.len(), 2, "{drift:?}");
        assert!(
            drift.iter().any(|d| d.contains("`tags` column")),
            "{drift:?}"
        );
        assert!(
            drift.iter().any(|d| d.contains("users_tags_idx")),
            "{drift:?}"
        );

        let clean = verify_schema(&schema, &[full.to_owned()], &url).expect("the check runs");
        assert!(clean.is_empty(), "{clean:?}");
    }
}
