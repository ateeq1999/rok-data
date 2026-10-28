//! The PostgreSQL smoke test written into the fixture crate.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;

use anyhow::Result;

use crate::generate::{Outcome, dependency_order, harness_header, subdirectories, up_migrations};
use crate::{db_dir, manifest_dir};

/// The names of the `pub async fn`s in a generated Rust file, in order.
pub(super) fn generated_functions(rust: &str) -> Vec<String> {
    rust.lines()
        .filter_map(|line| line.strip_prefix("pub async fn "))
        .filter_map(|rest| rest.split(['<', '(']).next())
        .map(str::to_owned)
        .collect()
}

/// Renders `src/db/smoke.rs`: the migrations of every migration-based
/// fixture in dependency order, and a `run` that calls `list`, `count` and
/// `search_after` for every generated table that has them.
pub(super) fn render_smoke(outcomes: &[Outcome]) -> Result<String> {
    let migration_based: BTreeMap<&str, &Outcome> = outcomes
        .iter()
        .filter(|outcome| outcome.fixture.migrations)
        .map(|outcome| (outcome.fixture.module.as_str(), outcome))
        .collect();

    let mut text = harness_header();
    text.push_str(
        "//! Smoke checks for the generated reads of every migration-based module fixture.\n\n\
         // Harness code, not generated code: it names each filter as `Default::default()`.\n\
         #![allow(clippy::default_trait_access)]\n\n",
    );
    text.push_str(
        "/// Every migration-based fixture's up migrations as `(path, sql)`, in dependency order.\n",
    );
    text.push_str("pub const MIGRATIONS: &[(&str, &str)] = &[\n");
    for module in dependency_order(&migration_based.keys().copied().collect())? {
        for path in up_migrations(&migration_based[module.as_str()].fixture.schema)? {
            let label = path
                .strip_prefix(manifest_dir())
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let absolute = path.to_string_lossy().into_owned();
            writeln!(text, "    ({label:?}, include_str!({absolute:?})),")?;
        }
    }
    text.push_str("];\n\n");

    text.push_str(
        "/// Runs every generated table's `search` chain as a caller does (`all`,\n\
         /// `count`, `cursor_paginate`) and returns one line per failed call.\n",
    );
    let mut calls = String::new();
    for (module, outcome) in &migration_based {
        let root = db_dir().join(&outcome.fixture.id);
        for table in subdirectories(&root)? {
            let read = root.join(&table).join("read.rs");
            if !read.is_file() {
                continue;
            }
            let source = fs::read_to_string(&read)?;
            let Some(search) = search_signature(&source) else {
                continue;
            };
            let model = source
                .lines()
                .find_map(|line| line.strip_prefix("impl "))
                .and_then(|rest| rest.strip_suffix(" {"))
                .expect("read.rs holds the model's `impl`");
            let path = format!("super::modules::{module}::{table}::models::{model}");
            // A tenant table's search takes the tenant first; the nil tenant
            // owns no rows, which still runs every statement.
            let tenant = match tenant_type(&search).as_deref() {
                None => "",
                Some("uuid::Uuid") => "uuid::Uuid::nil(), ",
                Some(_) => "uuid::Uuid::nil().into(), ",
            };
            let call = format!("{path}::search({tenant}Default::default())");
            let name = format!("{module}::{table}");
            if search.contains("-> Result<") {
                writeln!(
                    calls,
                    "    match {call} {{\n        Ok(query) => reads(&mut failures, \"{name}\", pool, query).await,\n        Err(err) => failures.push(format!(\"{name}::search: {{err}}\")),\n    }}"
                )?;
            } else {
                writeln!(
                    calls,
                    "    reads(&mut failures, \"{name}\", pool, {call}).await;"
                )?;
            }
        }
    }
    text.push_str("pub async fn run(pool: &sqlx::PgPool) -> Vec<String> {\n");
    if calls.is_empty() {
        text.push_str("    let _ = pool;\n    Vec::new()\n}\n\n");
    } else {
        text.push_str("    let mut failures = Vec::new();\n");
        text.push_str(&calls);
        text.push_str("    failures\n}\n\n");
    }
    text.push_str(
        "/// `query`'s first rows, its count and its first keyset page.\n\
         async fn reads<M: rok_data::Model>(\n\
         \x20   failures: &mut Vec<String>,\n\
         \x20   name: &str,\n\
         \x20   pool: &sqlx::PgPool,\n\
         \x20   query: rok_data::Select<M>,\n\
         ) {\n\
         \x20   let all = query.clone().limit(10).all(pool).await;\n\
         \x20   record(failures, &format!(\"{name}::all\"), all);\n\
         \x20   let count = query.clone().count(pool).await;\n\
         \x20   record(failures, &format!(\"{name}::count\"), count);\n\
         \x20   let page = query.cursor_paginate(pool, None, 10).await;\n\
         \x20   record(failures, &format!(\"{name}::cursor_paginate\"), page);\n\
         }\n\n",
    );
    text.push_str(
        "fn record<T, E>(failures: &mut Vec<String>, call: &str, result: Result<T, E>)\n\
         \x20where\n\
         \x20    E: std::fmt::Display,\n\
         {\n\
         \x20   if let Err(err) = result {\n\
         \x20       failures.push(format!(\"{call}: {err}\"));\n\
         \x20   }\n\
         }\n",
    );
    Ok(text)
}

/// The signature of `search` in a generated `read.rs`, from its name to its
/// return type, or `None` when the table has no search.
fn search_signature(rust: &str) -> Option<String> {
    let rest = rust.split("pub fn search(").nth(1)?;
    rest.split(" {\n").next().map(str::to_owned)
}

/// The type of the `rok` preset's tenant parameter in a `search`
/// signature (a raw id or a typed id), or `None` when it takes no tenant.
fn tenant_type(search: &str) -> Option<String> {
    let tenant = rok_data_sqlc::Conventions::rok()
        .tenant_column
        .expect("the rok preset has a tenant column");
    search
        .split(&format!("{tenant}: "))
        .nth(1)
        .and_then(|rest| rest.split([',', '\n']).next())
        .map(|tenant_type| tenant_type.trim().to_owned())
}
