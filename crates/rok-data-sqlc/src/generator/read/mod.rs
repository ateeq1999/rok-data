use crate::schema::{Column, Table};

use super::search::SearchApi;
use super::{
    HEADER, filter_call, keys, model_chain, param_name, param_type, push_chain, push_doc,
    push_fn_head, tenant_filter_call, types_import, use_list, use_paths,
};

pub fn render(table: &Table, tables: &[Table]) -> String {
    let api = SearchApi::for_table(table);
    let fallible = !super::search::filter_relationships(table, tables).is_empty();
    let mut output = format!("{HEADER}\n{}", search_imports(table, api));
    let mut models = vec![table.struct_name.clone()];
    models.extend(keys::key_items(table));
    models.sort();
    output.push_str(&use_list("super::models", &models));
    // rok chains need the Model trait statics in scope (the executor bound
    // stays fully qualified). After the `use_list`, so rustfmt's order —
    // super-paths first — stays stable across regeneration.
    output.push_str("use rok_data::Model;\n");

    if table.primary_key.is_empty() {
        output.push_str(
            "\n// No primary key detected; get_by_id, exists_by_id, and list_page are not generated\n// for this table, and list returns rows in no particular order.\n",
        );
    } else {
        render_get_by_id(&mut output, table);
        render_exists_by_id(&mut output, table);
    }
    for lookup in unique_lookups(table) {
        render_unique_lookup(&mut output, table, &lookup);
    }
    render_list(&mut output, table);
    if !table.primary_key.is_empty() {
        render_list_page(&mut output, table);
    }
    render_count(&mut output, table);
    if api.any() {
        render_search(&mut output, table, fallible);
    }

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

/// A `get_by_<cols>` lookup over one unique key other than the primary key.
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
            let fn_name = format!("get_by_{}", column_names.join("_and_"));
            // `get_by_id` already names the primary-key lookup.
            if (skip_primary_key == 1 && fn_name == "get_by_id") || names.contains(&fn_name) {
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

/// A read's doc comment, noting on a soft-delete table that deleted rows
/// are left out.
fn push_read_doc(output: &mut String, table: &Table, lines: &[&str]) {
    push_doc_with_note(output, table, lines, "Soft-deleted rows are left out.");
}

fn push_doc_with_note(output: &mut String, table: &Table, lines: &[&str], note: &str) {
    let mut lines = lines.to_vec();
    if table.soft_delete_column.is_some() {
        lines.push(note);
    }
    push_doc(output, 0, &lines);
}

fn render_unique_lookup(output: &mut String, table: &Table, lookup: &Lookup<'_>) {
    let described: Vec<String> = lookup
        .columns
        .iter()
        .map(|column| super::doc_name(&column.sql_name))
        .collect();
    output.push('\n');
    push_read_doc(
        output,
        table,
        &[&format!(
            "Fetches the row with the given {}, or `None` if there is none.",
            described.join(" and ")
        )],
    );
    let params: Vec<String> = lookup
        .columns
        .iter()
        .map(|column| format!("{}: {}", param_name(column), param_type(column)))
        .collect();
    push_fn_head(
        output,
        &lookup.fn_name,
        &keys::scoped_params(table, &params),
        &format!("Result<Option<{}>, rok_data::Error>", table.struct_name),
    );
    let mut calls = Vec::new();
    if let Some(call) = tenant_filter_call(table) {
        calls.push(call);
    }
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
    calls.push(".first(executor)".to_owned());
    calls.push(".await".to_owned());
    push_chain(
        output,
        4,
        "",
        &model_chain(&table.struct_name, &mut calls, 4, ""),
        &calls,
    );
    output.push_str("}\n");
}

/// `impl Row { fn search }`: the rows `filter` matches as a query chain,
/// which the caller orders and pages (`.order_by(..)`, `.paginate(..)`,
/// `.cursor_paginate(..)`).
fn render_search(output: &mut String, table: &Table, fallible: bool) {
    let name = &table.struct_name;
    output.push_str(&format!("\nimpl {name} {{\n"));
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
    output.push_str("    }\n}\n");
}

fn render_get_by_id(output: &mut String, table: &Table) {
    output.push('\n');
    push_read_doc(
        output,
        table,
        &["Fetches the row with the given primary key, or `None` if there is none."],
    );
    push_fn_head(
        output,
        "get_by_id",
        &keys::key_params(table),
        &format!("Result<Option<{}>, rok_data::Error>", table.struct_name),
    );
    let mut calls = Vec::new();
    if let Some(call) = tenant_filter_call(table) {
        calls.push(call);
    }
    let binds = keys::key_binds(table);
    for (column, bind) in table.key_columns().into_iter().zip(&binds) {
        calls.push(filter_call(table, column, bind));
    }
    calls.push(".first(executor)".to_owned());
    calls.push(".await".to_owned());
    push_chain(
        output,
        4,
        "",
        &model_chain(&table.struct_name, &mut calls, 4, ""),
        &calls,
    );
    output.push_str("}\n");
}

fn render_exists_by_id(output: &mut String, table: &Table) {
    output.push('\n');
    push_read_doc(
        output,
        table,
        &["Returns whether a row with the given primary key exists."],
    );
    push_fn_head(
        output,
        "exists_by_id",
        &keys::key_params(table),
        "Result<bool, rok_data::Error>",
    );
    let mut calls = Vec::new();
    if let Some(call) = tenant_filter_call(table) {
        calls.push(call);
    }
    let binds = keys::key_binds(table);
    for (column, bind) in table.key_columns().into_iter().zip(&binds) {
        calls.push(filter_call(table, column, bind));
    }
    calls.push(".count(executor)".to_owned());
    calls.push(".await".to_owned());
    calls.push(".map(|count| count > 0)".to_owned());
    push_chain(
        output,
        4,
        "",
        &model_chain(&table.struct_name, &mut calls, 4, ""),
        &calls,
    );
    output.push_str("}\n");
}

fn render_list(output: &mut String, table: &Table) {
    let keys = table.primary_key_columns();
    let doc = if keys.is_empty() {
        "Fetches every row, in no particular order."
    } else {
        "Fetches every row, ordered by primary key."
    };

    output.push('\n');
    push_read_doc(output, table, &[doc]);
    push_fn_head(
        output,
        "list",
        &keys::tenant_params(table),
        &format!("Result<Vec<{}>, rok_data::Error>", table.struct_name),
    );
    let mut calls = Vec::new();
    if let Some(call) = tenant_filter_call(table) {
        calls.push(call);
    }
    for column in &keys {
        calls.push(format!(
            ".order_by({}::{})",
            table.struct_name,
            column.rust_name.to_uppercase()
        ));
    }
    calls.push(".all(executor)".to_owned());
    calls.push(".await".to_owned());
    push_chain(
        output,
        4,
        "",
        &model_chain(&table.struct_name, &mut calls, 4, ""),
        &calls,
    );
    output.push_str("}\n");
}

fn render_list_page(output: &mut String, table: &Table) {
    let keys = table.primary_key_columns();
    output.push('\n');
    push_read_doc(
        output,
        table,
        &[
            "Fetches at most `limit` rows after skipping `offset`, ordered by primary key so",
            "consecutive pages neither repeat nor skip rows. Both values must be non-negative.",
        ],
    );
    push_fn_head(
        output,
        "list_page",
        &keys::scoped_params(table, &["limit: i64".to_owned(), "offset: i64".to_owned()]),
        &format!("Result<Vec<{}>, rok_data::Error>", table.struct_name),
    );
    let mut calls = Vec::new();
    if let Some(call) = tenant_filter_call(table) {
        calls.push(call);
    }
    for column in &keys {
        calls.push(format!(
            ".order_by({}::{})",
            table.struct_name,
            column.rust_name.to_uppercase()
        ));
    }
    calls.push(".limit(limit as u64)".to_owned());
    calls.push(".offset(offset as u64)".to_owned());
    calls.push(".all(executor)".to_owned());
    calls.push(".await".to_owned());
    push_chain(
        output,
        4,
        "",
        &model_chain(&table.struct_name, &mut calls, 4, ""),
        &calls,
    );
    output.push_str("}\n");
}

fn render_count(output: &mut String, table: &Table) {
    output.push('\n');
    push_read_doc(output, table, &["Counts every row."]);
    push_fn_head(
        output,
        "count",
        &keys::tenant_params(table),
        "Result<i64, rok_data::Error>",
    );
    let mut calls = vec![".count(executor)".to_owned(), ".await".to_owned()];
    if let Some(call) = tenant_filter_call(table) {
        calls.insert(0, call);
    }
    push_chain(
        output,
        4,
        "",
        &model_chain(&table.struct_name, &mut calls, 4, ""),
        &calls,
    );
    output.push_str("}\n");
}

#[cfg(test)]
mod tests;
