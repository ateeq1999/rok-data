//! What a generated query function returns.

use std::collections::BTreeSet;

use crate::describe::Description;
use crate::emit::Names;
use crate::ir::{Query, Schema};

use crate::naming;
use crate::queries::Shape;
use crate::types::{self, Feature};

/// What a generated function returns per row.
pub(crate) enum Returns {
    Model(String),
    Scalar(String),
    Row(String, Vec<(String, String, String)>),
    Nothing,
}

pub(crate) fn returns(
    query: &Query,
    description: &Description,
    shape: &Shape,
    schema: &Schema,
    names: &Names<'_>,
    features: &mut BTreeSet<Feature>,
) -> Result<Returns, String> {
    let columns = &description.columns;
    if columns.is_empty() {
        return Err("the query returns no columns; annotate it with `:exec`".to_owned());
    }
    // Every column of one table, in order: return its model.
    for table_name in &shape.reads {
        let Some(table) = schema.tables.iter().find(|t| &t.name == table_name) else {
            continue;
        };
        // Models decode by column name, so the order doesn't matter (columns
        // added by a later migration come last in the database).
        let same = table.columns.len() == columns.len()
            && table.columns.iter().all(|tc| {
                columns
                    .iter()
                    .any(|dc| tc.name == dc.name && (tc.nullable || dc.nullable != Some(true)))
            });
        if same {
            return Ok(Returns::Model(
                names.model_path(&table.name).unwrap_or_default(),
            ));
        }
    }
    let mut fields = Vec::new();
    let mut seen = BTreeSet::new();
    for column in columns {
        if column.name == "?column?" {
            return Err("give every computed column a name (`count(*) AS total`)".to_owned());
        }
        let (name, nullable) = match column.name.strip_suffix('!') {
            Some(name) => (name, false),
            None => match column.name.strip_suffix('?') {
                Some(name) => (name, true),
                None => (column.name.as_str(), column.nullable != Some(false)),
            },
        };
        let field = naming::snake_ident(name);
        if !seen.insert(field.clone()) {
            return Err(format!(
                "two result columns are named `{field}`; rename one with AS"
            ));
        }
        let ty = names
            .rust_type(
                &types::canonical_from_pg(&column.type_name),
                nullable,
                features,
            )
            .map_err(|e| format!("column `{name}`: {e}"))?;
        fields.push((field, column.name.clone(), ty));
    }
    if fields.len() == 1 {
        return Ok(Returns::Scalar(fields.remove(0).2));
    }
    Ok(Returns::Row(
        format!("{}Row", naming::pascal(&query.name)),
        fields,
    ))
}
