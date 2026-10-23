//! `destroy`, and `restore` and `delete_permanently` under soft delete.
//! The generated model's static method is `destroy` (not `delete`).

use crate::schema::{Column, Table};

use crate::generator::{
    filter_call, keys, model_chain, push_doc, push_fn_head, push_wide_chain, tenant_filter_call,
};

pub(super) fn render_destroy(output: &mut String, table: &Table, _key_columns: &[&Column]) {
    match table.soft_delete_column.as_deref() {
        Some(column) => {
            let column = crate::generator::sql::ident(column);
            render_key_statement(
                output,
                table,
                "destroy",
                &[
                    &format!(
                        "Soft-deletes the row with the given primary key by setting `{column}` to"
                    ),
                    "the current time, and returns the number of rows changed: `0` when there is no",
                    "such row or it is already deleted.",
                ],
                |calls| push_destroy_terminal(calls, ".delete(executor)"),
            );
            render_key_statement(
                output,
                table,
                "restore",
                &[
                    &format!(
                        "Restores the soft-deleted row with the given primary key by clearing `{column}`,"
                    ),
                    "and returns the number of rows changed: `0` when there is no such row or it is",
                    "not deleted.",
                ],
                |calls| push_destroy_terminal(calls, ".restore(executor)"),
            );
            render_key_statement(
                output,
                table,
                "delete_permanently",
                &[
                    "Removes the row with the given primary key from the table, deleted or not, and",
                    "returns the number of rows that existed.",
                ],
                |calls| push_destroy_terminal(calls, ".force_delete(executor)"),
            );
        }
        None => {
            render_key_statement(
                output,
                table,
                "destroy",
                &[
                    "Deletes the row with the given primary key and returns the number of rows that existed.",
                ],
                |calls| push_destroy_terminal(calls, ".delete(executor)"),
            );
        }
    }
}

/// The statement-specific terminal: plain and soft `destroy` go through the
/// model's `DELETE`/`UPDATE deleted_at = now()`; `restore` targets only
/// deleted rows; `delete_permanently` lifts the live-only scope first.
fn push_destroy_terminal(calls: &mut Vec<String>, terminal: &str) {
    if terminal == ".force_delete(executor)" {
        calls.push(".with_trashed()".to_owned());
    }
    calls.push(terminal.to_owned());
    calls.push(".await".to_owned());
}

/// A function taking the primary key that runs the model chain and returns
/// the number of rows affected.
fn render_key_statement(
    output: &mut String,
    table: &Table,
    name: &str,
    doc: &[&str],
    push_calls: impl FnOnce(&mut Vec<String>),
) {
    output.push('\n');
    push_doc(output, 0, doc);
    push_fn_head(
        output,
        name,
        &keys::key_params(table),
        "Result<u64, rok_data::Error>",
    );
    let mut calls = Vec::new();
    if let Some(call) = tenant_filter_call(table) {
        calls.push(call);
    }
    let binds = keys::key_binds(table);
    for (column, bind) in table.key_columns().into_iter().zip(&binds) {
        calls.push(filter_call(table, column, bind));
    }
    push_calls(&mut calls);
    push_wide_chain(
        output,
        4,
        "",
        &model_chain(&table.struct_name, &mut calls, 4, ""),
        &calls,
        "",
    );
    output.push_str("}\n")
}
