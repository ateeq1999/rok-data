//! The column and table comments a schema file carries.

use crate::ir::Schema;

/// `-- comment` lines above column definitions inside `CREATE TABLE`:
/// (column, doc lines, `rok:tenant`).
pub(crate) fn column_comments(sql: &str) -> Vec<(String, Vec<String>, bool)> {
    let mut out = Vec::new();
    let mut doc = Vec::new();
    let mut tenant = false;
    for line in sql.lines().skip(1) {
        let line = line.trim();
        if let Some(comment) = line.strip_prefix("--") {
            let comment = comment.trim();
            if comment == "rok:tenant" {
                tenant = true;
            } else {
                doc.push(comment.to_owned());
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if !doc.is_empty() || tenant {
            let first = line
                .split(|c: char| c.is_whitespace() || c == ',')
                .next()
                .unwrap_or_default();
            let column = first.trim_matches('"').to_owned();
            out.push((column, std::mem::take(&mut doc), tenant));
            tenant = false;
        }
    }
    out
}

/// `REFERENCES users` without columns points at the primary key.
pub(crate) fn resolve_references(schema: &mut Schema) {
    let keys: Vec<(String, Vec<String>)> = schema
        .tables
        .iter()
        .map(|t| (t.name.clone(), t.primary_key.clone()))
        .collect();
    for table in &mut schema.tables {
        for fk in &mut table.foreign_keys {
            if fk.ref_columns.is_empty()
                && let Some((_, key)) = keys.iter().find(|(name, _)| *name == fk.ref_table)
            {
                fk.ref_columns = key.clone();
            }
        }
    }
}

pub(crate) fn apply_comments(
    schema: &mut Schema,
    comments: &[(String, Option<String>, Option<String>)],
) {
    for (table, column, text) in comments {
        let Some(t) = schema.tables.iter_mut().find(|t| &t.name == table) else {
            continue;
        };
        let lines: Vec<String> = text
            .iter()
            .flat_map(|t| t.lines())
            .map(str::to_owned)
            .collect();
        match column {
            Some(c) => {
                if let Some(col) = t.columns.iter_mut().find(|col| &col.name == c) {
                    col.doc = lines;
                }
            }
            None => t.doc = lines,
        }
    }
}
