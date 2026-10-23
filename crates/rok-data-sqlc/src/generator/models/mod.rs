//! A table's `models.rs`: its row, its key type, its create input and its
//! update patch ([`inputs`]), the row's relation attributes ([`relations`]),
//! and `db/nullable.rs` ([`nullable`]).

mod conventions;
mod inferred;
mod inputs;
mod nullable;
mod relations;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod validate_tests;

use crate::schema::{Column, Table, mentions_generated_type};

use self::conventions::Conventions;
use self::inputs::{render_insert_input, render_update_patch};
use self::relations::Relations;
use super::{
    DeriveOptions, HEADER, doc_name, keys, push_derives, push_doc, serde_derives, sql, use_paths,
};

pub use self::nullable::render_nullable;

/// The doc line of a row, input, or patch field: the column it holds.
fn column_doc(column: &Column) -> String {
    format!("The {} column.", doc_name(&column.sql_name))
}

pub fn render(table: &Table, derives: DeriveOptions, tables: &[Table]) -> String {
    let mut output = format!("{HEADER}\n");
    let mut imports = keys::id_imports(table, &table.columns, true);
    imports.push("rok_data::Model".to_owned());
    // A column typed by another module's id (one wrapping an enum) does not
    // name the enum here; this module's own id does, in its definition.
    if table.columns.iter().any(|column| {
        mentions_generated_type(&column.rust_type)
            && column
                .typed_id
                .as_ref()
                .is_none_or(|id| id.module == table.module_name)
    }) {
        imports.push("super::super::types".to_owned());
    }
    let relations = Relations::collect(table, tables);
    imports.extend(relations.imports().iter().cloned());
    imports.sort();
    imports.dedup();
    if !imports.is_empty() {
        output.push_str(&use_paths(&imports));
        output.push('\n');
    }
    keys::render_key_type(&mut output, table, derives);
    let conventions = conventions::decide(table);
    render_row(&mut output, table, derives.serde, &conventions, &relations);
    keys::render_row_key(&mut output, table);

    let insert_columns = table.insert_columns();
    if !insert_columns.is_empty() {
        render_insert_input(&mut output, table, &insert_columns, derives);
    }

    let mut update_columns = table.update_columns();
    if !table.primary_key.is_empty() && !update_columns.is_empty() {
        render_update_patch(&mut output, table, &update_columns, derives);
    } else {
        update_columns.clear();
    }

    if derives.validate {
        let fields = insert_columns.into_iter().chain(update_columns);
        super::validate::push_decimal_checks(&mut output, fields);
    }
    output
}

fn render_row(
    output: &mut String,
    table: &Table,
    serde: bool,
    conventions: &Conventions,
    relations: &Relations,
) {
    let doc = format!("A row of the `{}` table.", table.display_name());
    push_doc(output, 0, &[&doc]);
    output.push_str("#[derive(Debug, Clone, PartialEq, Model)]\n");
    push_derives(output, serde_derives(serde));
    push_container_attrs(output, table, conventions);
    for attr in relations.container_attrs() {
        output.push_str(attr);
        output.push('\n');
    }
    push_scope_attrs(output, table);
    output.push_str(&format!("pub struct {} {{\n", table.struct_name));
    // A lone `id` key is the derive's inference; any other key is marked.
    let marks_key = !inferred::infers_primary_key(table);
    for column in &table.columns {
        push_doc(output, 4, &[&column_doc(column)]);
        push_field_attrs(output, column, marks_key, conventions, relations);
        output.push_str(&format!(
            "    pub {}: {},\n",
            column.rust_name,
            column.field_type()
        ));
    }
    output.push_str("}\n");
}

/// The `#[rok(...)]` container attributes of a row's derive: the table (with
/// its schema), and the conventions the row carries (workspace ADR 0015).
/// The markers the inference covers are not written: the column names say
/// it already. The opt-outs are written where the database owns the column.
fn push_container_attrs(output: &mut String, table: &Table, conventions: &Conventions) {
    let mut attrs = Vec::new();
    // The table is written only where the derive would infer another name.
    if !inferred::infers_table(table) {
        attrs.push(format!(
            "table = {}",
            sql::str_literal(&qualified_table(table))
        ));
    }
    if conventions.no_timestamps {
        attrs.push("no_timestamps".to_owned());
    }
    if conventions.no_soft_delete {
        attrs.push("no_soft_delete".to_owned());
    }
    if !attrs.is_empty() {
        output.push_str(&format!("#[rok({})]\n", attrs.join(", ")));
    }
}

/// The `#[rok(scopes(...))]` attribute of a row's derive: the scopes the
/// `[generate.scopes]` configuration declares for its table, each naming
/// the free function behind it (spec D7). A table without config entries
/// carries no attribute.
fn push_scope_attrs(output: &mut String, table: &Table) {
    for scope in &table.scopes {
        output.push_str(&format!(
            "#[rok(scopes({} = {}))]\n",
            scope.name, scope.path
        ));
    }
}

/// The `#[rok(...)]` attributes of one row field: the column's name when it
/// differs from the field, the primary key, generated-only columns ("read
/// but never written"), the soft-delete column's marker (when the
/// inference cannot name it), and the belongs-to relation the field's
/// foreign key holds.
fn push_field_attrs(
    output: &mut String,
    column: &Column,
    marks_key: bool,
    conventions: &Conventions,
    relations: &Relations,
) {
    if column.rust_name != column.sql_name {
        output.push_str(&format!(
            "    #[rok(column = {})]\n",
            sql::str_literal(&column.sql_name)
        ));
    }
    if column.is_primary_key && marks_key {
        output.push_str("    #[rok(primary_key)]\n");
    }
    if column.is_generated {
        output.push_str("    #[rok(generated)]\n");
    }
    if conventions
        .soft_delete
        .is_some_and(|soft_delete| soft_delete.sql_name == column.sql_name)
    {
        output.push_str("    #[rok(deleted_at)]\n");
    }
    if let Some(attr) = relations.field_attr(&column.sql_name) {
        output.push_str("    ");
        output.push_str(attr);
        output.push('\n');
    }
}

/// The table as rok-data's `#[rok(table = …)]` takes it: `schema.name`, or
/// just `name`, unquoted (rok-data quotes identifiers itself).
fn qualified_table(table: &Table) -> String {
    match &table.schema {
        Some(schema) => format!("{schema}.{}", table.sql_name),
        None => table.sql_name.clone(),
    }
}
