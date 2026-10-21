//! The shared schema artifact (engine ADR 0010): the engine's parsed schema in the
//! `rok-data-codegen` shape, so the resource-module generator and the crate
//! generator hold one round-trip truth (`schema.json`) between them.
//!
//! What the engine does not parse is absent from the artifact: plain
//! `CREATE INDEX`, other statements (views, extensions), `-- rok:` doc
//! comments, and the declaring file's module for enum types (the `module`
//! fields are metadata each generator fills in its own way, not schema
//! truth). Value-list enums of `CHECK`ed columns are not converted: only
//! database enum types are.

use anyhow::Context;

use rok_data_codegen as ir;
use rok_data_codegen::canonical_sql_type;

use crate::schema::{Column, EnumType, ForeignKey, Schema, Table};

/// The artifact's bytes: the parsed schema in the shared shape, rendered the
/// way the codegen's own snapshot file is (pretty JSON, trailing newline).
pub(crate) fn json(schema: &Schema) -> anyhow::Result<String> {
    let artifact = snapshot(schema);
    serde_json::to_string_pretty(&artifact)
        .map(|text| text + "\n")
        .context("failed to render the parsed schema to `schema.json`")
}

fn snapshot(schema: &Schema) -> ir::Schema {
    ir::Schema {
        enums: schema.enums.iter().map(convert_enum).collect(),
        tables: schema.tables.iter().map(convert_table).collect(),
        indexes: Vec::new(),
        extras: Vec::new(),
    }
}

fn convert_enum(enum_type: &EnumType) -> ir::EnumType {
    ir::EnumType {
        name: enum_type.display_name(),
        values: enum_type.variants.iter().map(|v| v.label.clone()).collect(),
        module: String::new(),
        doc: Vec::new(),
    }
}

fn convert_table(table: &Table) -> ir::Table {
    ir::Table {
        name: table.display_name(),
        module: table.module_name.clone(),
        doc: Vec::new(),
        columns: table.columns.iter().map(convert_column).collect(),
        primary_key: table.primary_key.clone(),
        uniques: table
            .key_constraints
            .iter()
            .filter(|constraint| constraint.columns != table.primary_key)
            .map(|constraint| ir::Unique {
                name: constraint.name.clone(),
                columns: constraint.columns.clone(),
            })
            .collect(),
        foreign_keys: table.foreign_keys.iter().map(convert_foreign_key).collect(),
        checks: Vec::new(),
    }
}

fn convert_column(column: &Column) -> ir::Column {
    ir::Column {
        name: column.sql_name.clone(),
        sql_type: canonical_sql_type(&column.sql_type),
        nullable: column.nullable,
        default: column.default.clone(),
        generated: column.generated.clone(),
        identity: column.identity.clone(),
        tenant: false,
        doc: Vec::new(),
    }
}

fn convert_foreign_key(key: &ForeignKey) -> ir::ForeignKey {
    ir::ForeignKey {
        name: key.name.clone(),
        columns: vec![key.column.clone()],
        ref_table: match &key.referenced_schema {
            Some(schema) => format!("{schema}.{}", key.referenced_table),
            None => key.referenced_table.clone(),
        },
        ref_columns: vec![key.referenced_column.clone()],
        on_delete: key.on_delete.clone(),
        on_update: key.on_update.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::parser::{SchemaSource, parse_sources};
    use std::path::Path;

    const SQL: &str = "CREATE TYPE user_status AS ENUM ('pending', 'active');\n\
        CREATE TABLE users (\n  id BIGINT PRIMARY KEY,\n  email TEXT NOT NULL,\n  name VARCHAR(255),\n  status user_status,\n  created_at TIMESTAMPTZ NOT NULL DEFAULT now()\n);\n\
        CREATE TABLE posts (\n  id BIGINT PRIMARY KEY,\n  author_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE\n);\n";

    fn engine_snapshot() -> ir::Schema {
        let parsed = parse_sources(&[SchemaSource {
            path: Path::new("0001_init.up.sql"),
            sql: SQL,
        }])
        .expect("the engine parses");
        snapshot(&parsed.schema)
    }

    fn codegen_snapshot() -> ir::Schema {
        let sources = [rok_data_codegen::SourceFile {
            path: "db/init.sql".to_owned(),
            module: "init".to_owned(),
            text: SQL.to_owned(),
        }];
        rok_data_codegen::parse(&sources)
            .expect("the codegen parses")
            .schema
    }

    #[test]
    fn the_artifact_matches_the_codegen_parse_of_the_same_sql() {
        let mut engine = engine_snapshot();
        let mut codegen = codegen_snapshot();
        // The `module` fields are per-generator metadata, not schema truth;
        // the engine fills enum modules with nothing (see the module docs).
        for table in engine.tables.iter_mut().chain(codegen.tables.iter_mut()) {
            table.module = String::new();
        }
        for enum_type in engine.enums.iter_mut().chain(codegen.enums.iter_mut()) {
            enum_type.module = String::new();
        }
        assert_eq!(engine, codegen);
    }

    #[test]
    fn the_json_is_pretty_and_trailing_newline() {
        let parsed = parse_sources(&[SchemaSource {
            path: Path::new("0001_init.up.sql"),
            sql: SQL,
        }])
        .expect("the engine parses");
        let text = json(&parsed.schema).expect("renders");
        assert!(text.starts_with("{\n  \"enums\":"));
        assert!(text.ends_with("}\n"));
    }
}
