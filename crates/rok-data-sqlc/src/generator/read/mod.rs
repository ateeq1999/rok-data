use crate::schema::{Column, Table};

use super::search::SearchApi;
use super::{
    HEADER, filter_call, keys, model_chain, param_name, param_type, push_doc, tenant_filter_call,
    types_import, use_list, use_paths,
};

/// Renders `read.rs`: `impl Row` with the reads the schema knows and the
/// runtime doesn't, each a query chain the caller runs (`.first(..)`,
/// `.exists(..)`, `.paginate(..)`): `by_primary_key`, one `by_<columns>` per unique
/// key, and `search`. Fetching by key, every row, or a count is the
/// runtime's (`Row::find`, `Row::all`, `Row::count`).
pub fn render(table: &Table, tables: &[Table]) -> String {
    let api = SearchApi::for_table(table);
    let fallible = !super::search::filter_relationships(table, tables).is_empty();
    let mut items = Vec::new();
    if !table.primary_key.is_empty() {
        items.push(render_by_primary_key(table));
    }
    for lookup in unique_lookups(table) {
        items.push(render_unique_lookup(table, &lookup));
    }
    if api.any() {
        items.push(render_search(table, fallible));
    }
    let mut output = format!("{HEADER}\n");
    if table.primary_key.is_empty() {
        output.push_str(
            "// No primary key detected: `by_primary_key` is not generated for this table, and its\n// rows come back in no particular order.\n",
        );
    }
    if items.is_empty() {
        return output;
    }
    if table.primary_key.is_empty() {
        output.push('\n');
    }
    output.push_str(&search_imports(table, api));
    let mut models = vec![table.struct_name.clone()];
    models.extend(keys::key_items(table));
    models.sort();
    output.push_str(&use_list("super::models", &models));
    // rok chains need the Model trait statics in scope. After the
    // `use_list`, so rustfmt's order — super-paths first — stays stable.
    output.push_str("use rok_data::Model;\n");
    output.push_str(&format!(
        "\nimpl {} {{\n{}}}\n",
        table.struct_name,
        items.join("\n")
    ));
    output
}

/// The `use` lines for the filter and enum types `read.rs` needs, sorted the
/// way rustfmt sorts them (`super::super::` before `super::filter`).
fn search_imports(table: &Table, api: SearchApi) -> String {
    let mut paths = Vec::new();
    // Key parameters and the keyset cursor use the key's type: an enum key
    // column's type, never a key struct's fields. Unique-key lookups take
    // each key column's value type.
    let lookups = unique_lookups(table);
    let lookup_columns = || {
        lookups
            .iter()
            .flat_map(|lookup| lookup.columns.iter().copied())
    };
    let mut value_types: Vec<String> = lookup_columns()
        .map(|column| column.value_type().to_owned())
        .collect();
    if !table.key_columns().is_empty() {
        value_types.push(keys::key_type(table));
    }
    value_types.extend(table.tenant().map(|tenant| tenant.value_type().to_owned()));
    if !types_import("super::super", value_types.iter().map(String::as_str)).is_empty() {
        paths.push("super::super::types".to_owned());
    }
    paths.extend(keys::id_imports(
        table,
        lookup_columns().chain(keys::param_columns(table)),
        true,
    ));
    if api.any() {
        paths.push(format!("super::filter::{}Filter", table.struct_name));
    }
    use_paths(&paths)
}

/// A `by_<cols>` lookup over one unique key other than the primary key.
pub(crate) struct Lookup<'a> {
    pub(crate) fn_name: String,
    pub(crate) columns: Vec<&'a Column>,
    /// The column a case-insensitive key compares ignoring case (its
    /// `lower(<column>)` entry), so the lookup compares
    /// `lower(<column>) = lower($n)`.
    pub(crate) lowered: Option<String>,
}

/// The table's unique-key lookups, in declaration order. A partial unique
/// index counts only when scoped to live rows by the soft-delete column. On
/// a tenant table the tenant parameter supplies the tenant column, so it is
/// left out of each key and of the lookup's name; a key left empty, or named
/// like an earlier lookup, gets none.
pub(crate) fn unique_lookups(table: &Table) -> Vec<Lookup<'_>> {
    let skip_primary_key = usize::from(!table.primary_key.is_empty());
    let mut names: Vec<String> = Vec::new();
    table
        .lookup_unique_keys(table.soft_delete_column.as_deref())
        .into_iter()
        .skip(skip_primary_key)
        .filter_map(|key| {
            let columns: Vec<&Column> = key
                .columns
                .iter()
                .map(|name| table.columns.iter().find(|column| column.sql_name == *name))
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .filter(|column| !table.is_tenant_column(column))
                .collect();
            if columns.is_empty() {
                return None;
            }
            let column_names: Vec<&str> = columns
                .iter()
                .map(|column| column.rust_name.trim_end_matches('_'))
                .collect();
            let fn_name = format!("by_{}", column_names.join("_and_"));
            // A column named `primary_key` would name the primary-key lookup.
            if (skip_primary_key == 1 && fn_name == "by_primary_key") || names.contains(&fn_name) {
                return None;
            }
            names.push(fn_name.clone());
            Some(Lookup {
                fn_name,
                columns,
                lowered: key.lowered,
            })
        })
        .collect()
}

/// A read's doc comment at the `impl`'s indent, noting on a soft-delete
/// table that deleted rows are left out.
fn push_read_doc(output: &mut String, table: &Table, lines: &[&str]) {
    let mut lines = lines.to_vec();
    if table.soft_delete_column.is_some() {
        lines.push("Soft-deleted rows are left out.");
    }
    push_doc(output, 4, &lines);
}

/// A chain-returning read: `#[must_use] pub fn {name}({params}) ->
/// Select<Row>`, its body the model chain of `calls`.
fn push_chain_read(
    output: &mut String,
    table: &Table,
    name: &str,
    params: &[String],
    calls: Vec<String>,
) {
    let row = &table.struct_name;
    output.push_str("    #[must_use]\n");
    let head = format!("pub fn {name}");
    super::push_signature(
        output,
        4,
        &head,
        params,
        &format!("rok_data::Select<{row}>"),
    );
    let mut calls = calls;
    let chain_head = model_chain(row, &mut calls, 8, "");
    super::push_wide_chain(output, 8, "", &chain_head, &calls, "");
    output.push_str("    }\n");
}

/// `by_primary_key`: the row with the given primary key (in the tenant), as a
/// chain: `.first(executor)` fetches it, `.exists(executor)` checks it.
fn render_by_primary_key(table: &Table) -> String {
    let mut output = String::new();
    push_read_doc(
        &mut output,
        table,
        &[
            "The row with the given primary key, as a query chain: `.first(executor)`",
            "fetches it, `.exists(executor)` checks it.",
        ],
    );
    let mut calls: Vec<String> = tenant_filter_call(table).into_iter().collect();
    let binds = keys::key_binds(table);
    for (column, bind) in table.key_columns().into_iter().zip(&binds) {
        calls.push(filter_call(table, column, bind));
    }
    push_chain_read(
        &mut output,
        table,
        "by_primary_key",
        &keys::key_params(table),
        calls,
    );
    output
}

fn render_unique_lookup(table: &Table, lookup: &Lookup<'_>) -> String {
    let described: Vec<String> = lookup
        .columns
        .iter()
        .map(|column| super::doc_name(&column.sql_name))
        .collect();
    let mut output = String::new();
    push_read_doc(
        &mut output,
        table,
        &[
            &format!(
                "The row with the given {}, as a query chain:",
                described.join(" and ")
            ),
            "`.first(executor)` fetches it.",
        ],
    );
    let params: Vec<String> = lookup
        .columns
        .iter()
        .map(|column| format!("{}: {}", param_name(column), param_type(column)))
        .collect();
    let mut calls: Vec<String> = tenant_filter_call(table).into_iter().collect();
    for column in &lookup.columns {
        let value = param_name(column);
        calls.push(
            if lookup.lowered.as_deref() == Some(column.sql_name.as_str()) {
                super::lowered_filter_call(column, &value)
            } else {
                filter_call(table, column, &value)
            },
        );
    }
    let params = keys::scoped_params(table, &params);
    push_chain_read(&mut output, table, &lookup.fn_name, &params, calls);
    output
}

/// `search`: the rows `filter` matches as a query chain, which the caller
/// orders and pages (`.order_by(..)`, `.paginate(..)`,
/// `.cursor_paginate(..)`).
fn render_search(table: &Table, fallible: bool) -> String {
    let name = &table.struct_name;
    let mut output = String::new();
    let output = &mut output;
    let mut doc = vec![
        "The rows matching `filter`, as a query chain: order and page it there,",
        "`.order_by(..)` and `.paginate(executor, page, per_page)`.",
    ];
    if table.soft_delete_column.is_some() {
        doc.push("Soft-deleted rows are left out unless `filter.include_deleted` is set.");
    }
    if fallible {
        doc.extend([
            "",
            "# Errors",
            "",
            "`rok_data::Error::InvalidQuery` when relationship filters nest more than",
            "`MAX_RELATIONSHIP_DEPTH` deep.",
        ]);
    }
    push_doc(output, 4, &doc);
    let select = format!("rok_data::Select<{name}>");
    let returns = if fallible {
        format!("Result<{select}, rok_data::Error>")
    } else {
        output.push_str("    #[must_use]\n");
        select
    };
    let params = keys::scoped_params(table, &[format!("filter: {name}Filter")]);
    super::push_signature(output, 4, "pub fn search", &params, &returns);
    let live = table.soft_delete_column.is_some();
    if live {
        output.push_str(&format!(
            "        let trashed = filter.{};\n",
            super::conventions::INCLUDE_DELETED
        ));
    }
    let condition = if fallible {
        "filter.condition()?"
    } else {
        "filter.condition()"
    };
    let mut calls: Vec<String> = tenant_filter_call(table).into_iter().collect();
    calls.push(format!(".filter({condition})"));
    let lead = if live { "let query = " } else { "" };
    let head = model_chain(name, &mut calls, 8, lead);
    match (live, fallible) {
        (true, _) => super::push_wide_chain(output, 8, lead, &head, &calls, ";"),
        (false, true) => super::push_ok_chain(output, 8, &head, &calls),
        (false, false) => super::push_wide_chain(output, 8, "", &head, &calls, ""),
    }
    match (live, fallible) {
        (true, true) => {
            output.push_str("        Ok(if trashed { query.with_trashed() } else { query })\n")
        }
        (true, false) => {
            output.push_str("        if trashed { query.with_trashed() } else { query }\n")
        }
        _ => {}
    }
    output.push_str("    }\n");
    output.clone()
}

#[cfg(test)]
mod tests;
