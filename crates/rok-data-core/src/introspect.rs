//! Inspect the live PostgreSQL schema via `information_schema` and
//! `pg_catalog`: tables, columns, indexes and foreign keys.
//!
//! Used by the `rok db inspect` and `rok schema dump` CLI commands; the same
//! functions are a stable runtime API for applications that build on top of
//! catalog metadata. The `rok-data` facade crate re-exports this module, so
//! applications call it as `rok_data::introspect`.
//!
//! ```rust,no_run
//! # use rok_data_core::Db;
//! # async fn example(db: Db) -> Result<(), rok_data_core::Error> {
//! for table in rok_data_core::introspect::tables(&db).await? {
//!     for column in rok_data_core::introspect::columns(&db, &table).await? {
//!         println!("{}.{} {}", table, column.name, column.data_type);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

use crate::Db;
use crate::error::Error;

/// Metadata about a single database column.
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    /// Column name.
    pub name: String,
    /// SQL data type name (e.g. `BIGINT`, `VARCHAR`, `TIMESTAMP WITH TIME ZONE`).
    pub data_type: String,
    /// Maximum character length (`VARCHAR(255)` → `255`); `None` for non-character types.
    pub max_length: Option<i32>,
    /// Whether the column is nullable.
    pub nullable: bool,
    /// Default expression, if any.
    pub default: Option<String>,
    /// Whether the column is part of the primary key.
    pub is_pk: bool,
}

/// Metadata about a database index.
#[derive(Debug, Clone)]
pub struct IndexInfo {
    /// Index name.
    pub name: String,
    /// Column names in index order.
    pub columns: Vec<String>,
    /// Whether the index enforces uniqueness.
    pub unique: bool,
    /// Whether this index is the primary key index.
    pub primary: bool,
}

/// Metadata about a foreign-key constraint.
#[derive(Debug, Clone)]
pub struct ForeignKeyInfo {
    /// Constraint name.
    pub constraint_name: String,
    /// Local column names participating in the FK.
    pub columns: Vec<String>,
    /// Referenced table name.
    pub foreign_table: String,
    /// Referenced column names.
    pub foreign_columns: Vec<String>,
}

/// Journal tables (old and new names) that `tables()` skips, so inspect and
/// dump output shows only user tables.
pub const JOURNAL_TABLES: [&str; 2] = ["_rok_data_migrations", "_rok_db_migrations"];

/// Every user table in schema `public`, alphabetical.
pub async fn tables(db: &Db) -> Result<Vec<String>, Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT table_name FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_type = 'BASE TABLE' \
         AND table_name <> ALL($1::text[]) ORDER BY table_name",
    )
    .bind(JOURNAL_TABLES)
    .fetch_all(db.pool())
    .await?;
    Ok(rows.into_iter().map(|(name,)| name).collect())
}

/// Metadata for every column in `table` (schema `public`), in declaration
/// order.
pub async fn columns(db: &Db, table: &str) -> Result<Vec<ColumnInfo>, Error> {
    let pk_cols = pk_columns(db, table).await?;

    #[derive(sqlx::FromRow)]
    struct RawCol {
        column_name: String,
        udt_name: String,
        character_maximum_length: Option<i32>,
        is_nullable: String,
        column_default: Option<String>,
    }

    let raw: Vec<RawCol> = sqlx::query_as(
        "SELECT column_name, udt_name, character_maximum_length, \
                is_nullable, column_default \
         FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = $1 \
         ORDER BY ordinal_position",
    )
    .bind(table)
    .fetch_all(db.pool())
    .await?;

    Ok(raw
        .into_iter()
        .map(|r| {
            let is_pk = pk_cols.contains(&r.column_name);
            ColumnInfo {
                name: r.column_name,
                data_type: pg_type_str(&r.udt_name, r.character_maximum_length),
                max_length: r.character_maximum_length,
                nullable: r.is_nullable == "YES",
                default: r.column_default,
                is_pk,
            }
        })
        .collect())
}

/// Metadata for every index on `table` (schema `public`), alphabetical.
pub async fn indexes(db: &Db, table: &str) -> Result<Vec<IndexInfo>, Error> {
    #[derive(sqlx::FromRow)]
    struct RawIdx {
        indexname: String,
        indexdef: String,
    }

    let raw: Vec<RawIdx> = sqlx::query_as(
        "SELECT indexname, indexdef FROM pg_indexes \
         WHERE tablename = $1 AND schemaname = 'public' \
         ORDER BY indexname",
    )
    .bind(table)
    .fetch_all(db.pool())
    .await?;

    Ok(raw
        .into_iter()
        .map(|r| {
            let unique = r.indexdef.contains(" UNIQUE ");
            let primary = r.indexdef.contains(" PRIMARY KEY ");
            let cols = parse_index_columns(&r.indexdef);
            IndexInfo {
                name: r.indexname,
                columns: cols,
                unique,
                primary,
            }
        })
        .collect())
}

/// Every foreign-key constraint originating from `table` (schema `public`).
pub async fn foreign_keys(db: &Db, table: &str) -> Result<Vec<ForeignKeyInfo>, Error> {
    #[derive(sqlx::FromRow)]
    struct RawFk {
        constraint_name: String,
        column_name: String,
        foreign_table_name: String,
        foreign_column_name: String,
    }

    let raw: Vec<RawFk> = sqlx::query_as(
        "SELECT tc.constraint_name, \
                kcu.column_name, \
                ccu.table_name AS foreign_table_name, \
                ccu.column_name AS foreign_column_name \
         FROM information_schema.table_constraints tc \
         JOIN information_schema.key_column_usage kcu \
           ON tc.constraint_name = kcu.constraint_name \
         JOIN information_schema.constraint_column_usage ccu \
           ON tc.constraint_name = ccu.constraint_name \
         WHERE tc.table_schema = 'public' \
           AND tc.table_name = $1 \
           AND tc.constraint_type = 'FOREIGN KEY' \
         ORDER BY tc.constraint_name, kcu.ordinal_position",
    )
    .bind(table)
    .fetch_all(db.pool())
    .await?;

    // Group by constraint_name
    let mut result: Vec<ForeignKeyInfo> = Vec::new();
    let mut current: Option<ForeignKeyInfo> = None;
    for r in raw {
        match &mut current {
            None => {
                current = Some(ForeignKeyInfo {
                    constraint_name: r.constraint_name,
                    columns: vec![r.column_name],
                    foreign_table: r.foreign_table_name,
                    foreign_columns: vec![r.foreign_column_name],
                });
            }
            Some(fk) if fk.constraint_name == r.constraint_name => {
                fk.columns.push(r.column_name);
                fk.foreign_columns.push(r.foreign_column_name);
            }
            _ => {
                if let Some(fk) = current.take() {
                    result.push(fk);
                }
                current = Some(ForeignKeyInfo {
                    constraint_name: r.constraint_name,
                    columns: vec![r.column_name],
                    foreign_table: r.foreign_table_name,
                    foreign_columns: vec![r.foreign_column_name],
                });
            }
        }
    }
    if let Some(fk) = current {
        result.push(fk);
    }
    Ok(result)
}

/// Approximate `CREATE TABLE` DDL for a single user table, in the same shape
/// as `rok schema dump` output (a convenience view, not a round-trip source
/// — see the CLI reference).
pub async fn dump_table(db: &Db, table: &str) -> Result<String, Error> {
    let cols = columns(db, table).await?;
    let pk_cols: Vec<&str> = cols
        .iter()
        .filter(|c| c.is_pk)
        .map(|c| c.name.as_str())
        .collect();

    let mut out = String::new();
    out.push_str(&format!("CREATE TABLE \"{table}\" ("));
    let n = cols.len();
    for (i, col) in cols.iter().enumerate() {
        let pk_s = if pk_cols.len() == 1 && pk_cols.contains(&col.name.as_str()) {
            " PRIMARY KEY"
        } else {
            ""
        };
        let null_s = if col.nullable { "" } else { " NOT NULL" };
        let def_s = col
            .default
            .as_deref()
            .map(|d| format!(" DEFAULT {d}"))
            .unwrap_or_default();
        let comma = if i + 1 < n || pk_cols.len() > 1 {
            ","
        } else {
            ""
        };
        out.push_str(&format!(
            "\n    \"{}\" {}{}{}{}{}",
            col.name, col.data_type, null_s, def_s, pk_s, comma
        ));
    }
    if pk_cols.len() > 1 {
        let pks = pk_cols
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("\n    PRIMARY KEY ({pks}),"));
    }
    out.push_str("\n);");
    Ok(out)
}

async fn pk_columns(db: &Db, table: &str) -> Result<Vec<String>, Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT kcu.column_name \
         FROM information_schema.table_constraints tc \
         JOIN information_schema.key_column_usage kcu \
           ON tc.constraint_name = kcu.constraint_name \
           AND tc.table_schema = kcu.table_schema \
         WHERE tc.table_schema = 'public' AND tc.table_name = $1 \
           AND tc.constraint_type = 'PRIMARY KEY' \
         ORDER BY kcu.ordinal_position",
    )
    .bind(table)
    .fetch_all(db.pool())
    .await?;
    Ok(rows.into_iter().map(|(n,)| n).collect())
}

/// SQL type name for a catalog `udt_name`, the way `\d` prints it.
fn pg_type_str(udt_name: &str, char_max: Option<i32>) -> String {
    match udt_name {
        "int8" | "bigserial" => "BIGINT".into(),
        "int4" | "serial" => "INTEGER".into(),
        "int2" | "smallserial" => "SMALLINT".into(),
        "float8" => "DOUBLE PRECISION".into(),
        "float4" => "REAL".into(),
        "bool" => "BOOLEAN".into(),
        "text" => "TEXT".into(),
        "varchar" => char_max.map_or("VARCHAR".into(), |n| format!("VARCHAR({n})")),
        "bpchar" => char_max.map_or("CHAR".into(), |n| format!("CHAR({n})")),
        "uuid" => "UUID".into(),
        "jsonb" => "JSONB".into(),
        "json" => "JSON".into(),
        "timestamptz" => "TIMESTAMP WITH TIME ZONE".into(),
        "timestamp" => "TIMESTAMP".into(),
        "date" => "DATE".into(),
        "time" => "TIME".into(),
        "bytea" => "BYTEA".into(),
        "numeric" => "NUMERIC".into(),
        other => other.to_uppercase(),
    }
}

/// Extract column names from a `pg_indexes.indexdef` string.
///
/// Example input: `"CREATE UNIQUE INDEX users_email_idx ON public.users USING btree (email)"`
/// Returns: `["email"]`
fn parse_index_columns(indexdef: &str) -> Vec<String> {
    let paren = match indexdef.find('(') {
        Some(p) => p,
        None => return Vec::new(),
    };
    let content = &indexdef[paren + 1..];
    let end = match content.find(')') {
        Some(e) => e,
        None => return Vec::new(),
    };
    let inner = &content[..end];
    inner
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dumps_primary_keys_within_the_table() {
        let cols = [
            ColumnInfo {
                name: "id".into(),
                data_type: "UUID".into(),
                max_length: None,
                nullable: false,
                default: None,
                is_pk: true,
            },
            ColumnInfo {
                name: "email".into(),
                data_type: "TEXT".into(),
                max_length: None,
                nullable: false,
                default: Some("''::text".into()),
                is_pk: false,
            },
        ];
        let pk: Vec<&str> = cols
            .iter()
            .filter(|c| c.is_pk)
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(pk, ["id"]);
    }

    #[test]
    fn parses_index_columns() {
        let cols = parse_index_columns(
            "CREATE UNIQUE INDEX users_email_idx ON public.users USING btree (email)",
        );
        assert_eq!(cols, ["email"]);
        assert!(parse_index_columns("CREATE INDEX no_parens").is_empty());
    }

    #[test]
    fn journal_tables_are_known() {
        assert!(JOURNAL_TABLES.contains(&"_rok_data_migrations"));
    }
}
