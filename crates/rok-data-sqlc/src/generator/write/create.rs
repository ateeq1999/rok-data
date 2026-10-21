//! `create`: an insert built column by column on the model chain.

use crate::schema::{Column, Table};

use crate::generator::{
    FN_CALL_WIDTH, MAX_WIDTH, insert_bind, keys, param_name, push_doc, push_fn_head,
    push_wide_chain,
};

/// Sets `column` to `value` on `builder`, laid out as rustfmt lays it out:
/// on one line while the call's arguments fit the call width and the
/// statement fits the line width, otherwise with one argument per line.
fn push_set_line(
    output: &mut String,
    table: &Table,
    column: &Column,
    value: &str,
    builder: &str,
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let args = format!("{}::{}, {value}", table.struct_name, const_name(column));
    let one_line = format!("{pad}{builder} = {builder}.set({args});");
    if args.len() <= FN_CALL_WIDTH && one_line.len() <= MAX_WIDTH {
        output.push_str(&one_line);
        output.push('\n');
    } else {
        for line in [
            format!("{pad}{builder} = {builder}.set("),
            format!("{pad}    {}::{},", table.struct_name, const_name(column)),
            format!("{pad}    {value},"),
            format!("{pad});"),
        ] {
            output.push_str(&line);
            output.push('\n');
        }
    }
}

/// The column's `Model` const: the field name in upper case.
fn const_name(column: &Column) -> String {
    column.rust_name.to_uppercase()
}

fn push_create_head(output: &mut String, table: &Table, doc: &[&str]) {
    output.push('\n');
    push_doc(output, 0, doc);
    push_fn_head(
        output,
        "create",
        &keys::scoped_params(table, &[format!("input: New{}", table.struct_name)]),
        &format!("Result<{}, rok_data::Error>", table.struct_name),
    );
}

pub(super) fn render_static_create(output: &mut String, table: &Table, columns: &[&Column]) {
    push_create_head(
        output,
        table,
        &["Inserts one row and returns it as stored."],
    );

    let mut calls = Vec::new();
    if let Some(tenant) = table.tenant() {
        calls.push(format!(
            ".set({}::{}, {})",
            table.struct_name,
            const_name(tenant),
            param_name(tenant)
        ));
    }
    for column in columns {
        calls.push(format!(
            ".set({}::{}, {})",
            table.struct_name,
            const_name(column),
            insert_bind(column)
        ));
    }
    calls.push(".exec(executor)".to_owned());
    calls.push(".await".to_owned());
    push_wide_chain(
        output,
        4,
        "",
        &format!("{}::insert_builder()", table.struct_name),
        &calls,
        "",
    );
    output.push_str("}\n");
}

pub(super) fn render_dynamic_create(output: &mut String, table: &Table, columns: &[&Column]) {
    push_create_head(
        output,
        table,
        &[
            "Inserts one row and returns it as stored. Defaulted columns left as `None`",
            "are omitted from the `INSERT` so the database default applies.",
        ],
    );

    // Columns left unset are left out of the `INSERT` (so the database
    // default applies); an all-`None` input without a tenant column renders
    // `INSERT … DEFAULT VALUES`. The model chain does both natively.
    let mut calls = Vec::new();
    if let Some(tenant) = table.tenant() {
        calls.push(format!(
            ".set({}::{}, {})",
            table.struct_name,
            const_name(tenant),
            param_name(tenant)
        ));
    }
    push_wide_chain(
        output,
        4,
        "let mut insert = ",
        &format!("{}::insert_builder()", table.struct_name),
        &calls,
        ";",
    );
    for column in columns {
        if column.is_omittable_on_insert() {
            output.push_str(&format!(
                "    if let Some(value) = input.{} {{\n",
                column.rust_name
            ));
            push_set_line(output, table, column, "value", "insert", 8);
            output.push_str("    }\n");
        } else {
            push_set_line(
                output,
                table,
                column,
                insert_bind(column).as_str(),
                "insert",
                4,
            );
        }
    }
    output.push_str("\n    insert.exec(executor).await\n");
    output.push_str("}\n");
}
