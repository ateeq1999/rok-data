//! `rok db check`, `inspect`, `create`, `drop`.

use std::process::ExitCode;

use crate::context;
use crate::output::json_line;

/// `rok db check`: connect, ping, report server version and pool settings.
///
/// New in the consolidated CLI — the four predecessor repositories had no
/// self-check command, and every migration/generation workflow benefits from
/// a usable connection diagnostic before it runs.
pub(crate) async fn check(database_url: Option<&str>, json: bool) -> Result<ExitCode, String> {
    let connection = context::Connection::resolve(database_url)?;
    let db = connection.probe_into_db().await?;
    db.ping().await.map_err(|e| e.to_string())?;
    let stats = db.stats();
    let version: String = rok_data::raw("SELECT version()")
        .scalar(&db)
        .await
        .map_err(|e| e.to_string())?;
    if json {
        json_line(serde_json::json!({
            "ok": true,
            "version": version,
            "pool": {
                "connections": stats.size,
                "idle": stats.idle,
                "max_connections": stats.max_connections,
            },
        }));
    } else {
        println!("ok: connected to {url}", url = connection.url_masked());
        println!(
            "pool: {} of {} connections",
            stats.size, stats.max_connections
        );
        println!("server: {}", version.lines().next().unwrap_or(""));
    }
    Ok(ExitCode::SUCCESS)
}

/// `rok db inspect`: list user tables, or everything about one table.
pub(crate) async fn inspect(
    table: Option<&str>,
    database_url: Option<&str>,
    json: bool,
) -> Result<ExitCode, String> {
    let connection = context::Connection::resolve(database_url)?;
    let db = connection.probe_into_db().await?;
    match table {
        None => {
            let tables = rok_data::introspect::tables(&db)
                .await
                .map_err(|e| e.to_string())?;
            if json {
                json_line(serde_json::json!({ "tables": tables }));
            } else {
                for table in tables {
                    println!("{table}");
                }
            }
        }
        Some(table) => {
            let columns = rok_data::introspect::columns(&db, table)
                .await
                .map_err(|e| e.to_string())?;
            let indexes = rok_data::introspect::indexes(&db, table)
                .await
                .map_err(|e| e.to_string())?;
            let foreign_keys = rok_data::introspect::foreign_keys(&db, table)
                .await
                .map_err(|e| e.to_string())?;
            if columns.is_empty() && json {
                return Err(format!("no table `{table}` in schema `public`"));
            }
            if json {
                let record = serde_json::json!({
                    "table": table,
                    "columns": columns.iter().map(|c| serde_json::json!({
                        "name": c.name,
                        "type": c.data_type,
                        "nullable": c.nullable,
                        "default": c.default,
                        "primary_key": c.is_pk,
                    })).collect::<Vec<_>>(),
                    "indexes": indexes.iter().map(|i| serde_json::json!({
                        "name": i.name,
                        "columns": i.columns,
                        "unique": i.unique,
                        "primary": i.primary,
                    })).collect::<Vec<_>>(),
                    "foreign_keys": foreign_keys.iter().map(|f| serde_json::json!({
                        "name": f.constraint_name,
                        "columns": f.columns,
                        "references": {
                            "table": f.foreign_table,
                            "columns": f.foreign_columns,
                        },
                    })).collect::<Vec<_>>(),
                });
                json_line(record);
            } else {
                for column in &columns {
                    let pk = if column.is_pk { " pk" } else { "" };
                    let nullable = if column.nullable { " null" } else { "" };
                    println!("{} {}{pk}{nullable}", column.name, column.data_type);
                }
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// `rok db create <DBNAME>`: creates a database on the target server.
pub(crate) async fn create(
    name: &str,
    database_url: Option<&str>,
    server: Option<&str>,
    json: bool,
) -> Result<ExitCode, String> {
    // Resolve connection: --database-url > --server > DATABASE_URL
    let connection = if let Some(url) = database_url {
        context::Connection::resolve(Some(url))?
    } else if let Some(server_name) = server {
        context::Connection::resolve_with_server(server_name)?
    } else {
        context::Connection::resolve(database_url)?
    };

    // Connect to the admin database (postgres) to create the new database
    let db = connection.probe_into_db().await?;

    // CREATE DATABASE needs autocommit (outside transaction)
    let sql = format!(r#"CREATE DATABASE "{}""#, name);
    rok_data::raw(&sql)
        .execute(&db)
        .await
        .map_err(|e| format!("failed to create database: {e}"))?;

    if json {
        json_line(crate::output::db_create_json(name, server));
    } else if let Some(server_name) = server {
        println!("Database '{name}' created on server '{server_name}'.");
    } else {
        println!("Database '{name}' created.");
    }

    Ok(ExitCode::SUCCESS)
}

/// `rok db drop <DBNAME>`: drops a database on the target server.
pub(crate) async fn drop(
    name: &str,
    database_url: Option<&str>,
    server: Option<&str>,
    yes: bool,
    force: bool,
    json: bool,
) -> Result<ExitCode, String> {
    // Resolve connection: --database-url > --server > DATABASE_URL
    let connection = if let Some(url) = database_url {
        context::Connection::resolve(Some(url))?
    } else if let Some(server_name) = server {
        context::Connection::resolve_with_server(server_name)?
    } else {
        context::Connection::resolve(database_url)?
    };

    // Project database protection: refuse to drop the project's own database
    // unless --force is given.
    let project_db = context::extract_project_db_name(database_url);
    if !force
        && let Some(ref project_db) = project_db
        && project_db.eq_ignore_ascii_case(name)
    {
        return Err(
            "refusing to drop the project's own database (DATABASE_URL). Use --force to override."
                .to_owned(),
        );
    }

    // Typed-name confirmation (unless --yes)
    if !yes {
        use std::io::{Write, stdin, stdout};
        print!("Type the database name to confirm: ");
        stdout().flush().map_err(|e| e.to_string())?;

        let mut input = String::new();
        stdin().read_line(&mut input).map_err(|e| e.to_string())?;

        if input.trim() != name {
            return Err("confirmation failed: database name does not match".to_owned());
        }
    }

    // Connect to the admin database (postgres) to drop the database
    let db = connection.probe_into_db().await?;

    // DROP DATABASE needs autocommit (outside transaction)
    let sql = format!(r#"DROP DATABASE "{}""#, name);
    rok_data::raw(&sql)
        .execute(&db)
        .await
        .map_err(|e| format!("failed to drop database: {e}"))?;

    if json {
        json_line(crate::output::db_drop_json(name, server));
    } else {
        println!("Database '{name}' dropped.");
    }

    Ok(ExitCode::SUCCESS)
}
