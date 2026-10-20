//! The warnings a query raises: a missing tenant or soft-delete filter.

use crate::ir::{Query, Schema};

use crate::queries::Shape;

pub(crate) fn scope_warnings(
    query: &Query,
    shape: &Shape,
    schema: &Schema,
    warnings: &mut Vec<String>,
) {
    let text = query.sql.to_ascii_lowercase();
    for table in schema
        .tables
        .iter()
        .filter(|t| shape.reads.contains(&t.name))
    {
        for column in table.columns.iter().filter(|c| c.tenant) {
            if !text.contains(&column.name.to_ascii_lowercase()) {
                warnings.push(format!(
                    "{}: `{}` reads tenant table `{}` without filtering on `{}`",
                    query.location, query.name, table.name, column.name
                ));
            }
        }
        let soft_delete = table.column("deleted_at").is_some_and(|c| c.nullable);
        let selects =
            text.trim_start().starts_with("select") || text.trim_start().starts_with("with");
        if soft_delete && selects && !text.contains("deleted_at") {
            warnings.push(format!(
                "{}: `{}` reads soft-delete table `{}` without filtering on `deleted_at`",
                query.location, query.name, table.name
            ));
        }
    }
}
