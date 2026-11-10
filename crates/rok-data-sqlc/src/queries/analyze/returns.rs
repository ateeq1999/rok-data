//! `-- returns: <Type>`: a query returns an existing row type instead of
//! its own `{Name}Row`.
//!
//! - **Another query's row type** of the same file (`FindCheckoutRow`):
//!   the columns must match it in name, order, type and nullability.
//! - **A model** of this run (`Checkout`): the columns must be the table's,
//!   in order, of the model's field types.
//! - **A hand-written type by path** (`crate::features::payouts::dto::
//!   PayoutSummary`): the query decodes into it (it derives
//!   `sqlx::FromRow`). The engine can't see its fields, so the compiler and
//!   the row decode catch a mismatch, not `rok gen`.
//!
//! Identical column lists are never merged without the annotation.

use std::collections::{HashMap, HashSet};

use super::{OutputColumn, ResultShape};
use crate::schema::Table;

/// The shape `target` names for a query whose own result is `result`, or
/// why it can't be. `rows` are the file's generated row types by name.
pub(super) fn resolve(
    target: &str,
    query: &str,
    result: &ResultShape,
    rows: &HashMap<String, Vec<OutputColumn>>,
    tables: &[Table],
    generated_modules: &HashSet<String>,
) -> Result<ResultShape, String> {
    let columns = match result {
        ResultShape::Row(columns) => columns.clone(),
        ResultShape::Model {
            module,
            struct_name,
        } => {
            if struct_name == target {
                return Ok(result.clone());
            }
            let table = tables
                .iter()
                .find(|table| &table.module_name == module)
                .ok_or_else(|| format!("the model `{struct_name}` is not generated"))?;
            model_columns(table)
        }
        ResultShape::None => {
            return Err(format!("`-- returns: {target}`: the query returns no rows"));
        }
        ResultShape::Scalar(rust_type) => {
            return Err(format!(
                "`-- returns: {target}`: the query returns one column (`{rust_type}`), not a row"
            ));
        }
        ResultShape::Shared { .. } | ResultShape::External(_) => return Ok(result.clone()),
    };
    if target.contains("::") {
        return Ok(ResultShape::External(target.to_owned()));
    }
    if target == format!("{query}Row") {
        return Err(format!(
            "`-- returns: {target}` names the query's own row type; leave it out"
        ));
    }
    if let Some(expected) = rows.get(target) {
        compare(target, expected, &columns)?;
        return Ok(ResultShape::Shared {
            name: target.to_owned(),
            columns,
        });
    }
    if let Some(table) = tables
        .iter()
        .find(|table| table.struct_name == target && generated_modules.contains(&table.module_name))
    {
        compare(target, &model_columns(table), &columns)?;
        return Ok(ResultShape::Model {
            module: table.module_name.clone(),
            struct_name: table.struct_name.clone(),
        });
    }
    Err(format!(
        "`-- returns: {target}`: no query of this file returns `{target}` and no model of \
         this run is named `{target}`; a hand-written type is named by its path \
         (`crate::…::{target}`)"
    ))
}

/// A model's columns as a query would return them.
fn model_columns(table: &Table) -> Vec<OutputColumn> {
    table
        .columns
        .iter()
        .map(|column| OutputColumn {
            sql_name: column.sql_name.clone(),
            rust_name: column.rust_name.clone(),
            rust_type: column.field_type(),
        })
        .collect()
}

/// Every difference between `expected`, the type `target`'s columns, and
/// the query's `actual` columns: a missing, extra, reordered or retyped
/// column.
fn compare(target: &str, expected: &[OutputColumn], actual: &[OutputColumn]) -> Result<(), String> {
    let mut differences = Vec::new();
    for (index, want) in expected.iter().enumerate() {
        match actual.get(index) {
            Some(got) if got.sql_name != want.sql_name => differences.push(format!(
                "column {} is `{}` here and `{}` in `{target}`",
                index + 1,
                got.sql_name,
                want.sql_name
            )),
            Some(got) if got.rust_type != want.rust_type => differences.push(format!(
                "column `{}` is `{}` here and `{}` in `{target}`",
                got.sql_name, got.rust_type, want.rust_type
            )),
            Some(_) => {}
            None => differences.push(format!(
                "column `{}` of `{target}` is missing here",
                want.sql_name
            )),
        }
    }
    for extra in actual.iter().skip(expected.len()) {
        differences.push(format!("column `{}` is not in `{target}`", extra.sql_name));
    }
    if differences.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "`-- returns: {target}`: the columns differ: {}",
            differences.join("; ")
        ))
    }
}
