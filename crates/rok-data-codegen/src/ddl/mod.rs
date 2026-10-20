//! Migrations: the SQL that turns one schema into another, with its undo.

use crate::ir::{Check, Column, EnumType, Extra, ForeignKey, Schema, Table, Unique};
use crate::naming;
use crate::parse::quote;

mod rename;

pub use rename::Rename;
pub(crate) use rename::diff;

/// One step of a migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Op {
    pub(crate) up: String,
    /// Empty when the step can't be undone.
    pub(crate) down: String,
    /// Why the step may lose data or fail on existing rows.
    pub(crate) destructive: Option<String>,
}

impl Op {
    fn new(up: impl Into<String>, down: impl Into<String>) -> Self {
        Self {
            up: up.into(),
            down: down.into(),
            destructive: None,
        }
    }

    fn destructive(mut self, why: impl Into<String>) -> Self {
        self.destructive = Some(why.into());
        self
    }
}
impl Op {
    fn destructive_if(self, condition: bool, why: impl Into<String>) -> Self {
        if condition {
            self.destructive(why)
        } else {
            self
        }
    }
}

fn has_extra(schema: &Schema, extra: &Extra) -> bool {
    schema.extras.iter().any(|e| e.up == extra.up)
}

fn enum_changes(old: &EnumType, new: &EnumType) -> Result<Vec<Op>, String> {
    let kept: Vec<&String> = new
        .values
        .iter()
        .filter(|v| old.values.contains(v))
        .collect();
    if kept.len() != old.values.len() || kept.iter().zip(&old.values).any(|(a, b)| *a != b) {
        return Err(format!(
            "enum `{}` lost or reordered values; PostgreSQL can only add values. \
             Write that change as a hand-made migration",
            old.name
        ));
    }
    let mut ops = Vec::new();
    for (i, value) in new.values.iter().enumerate() {
        if old.values.contains(value) {
            continue;
        }
        let position = match new.values[..i]
            .iter()
            .rev()
            .find(|v| old.values.contains(v))
        {
            Some(prev) => format!(" AFTER {}", literal(prev)),
            None => match new.values[i + 1..].iter().find(|v| old.values.contains(v)) {
                Some(next) => format!(" BEFORE {}", literal(next)),
                None => String::new(),
            },
        };
        // PostgreSQL can't remove an enum value, so this step has no undo.
        ops.push(Op::new(
            format!(
                "ALTER TYPE {} ADD VALUE {}{position}",
                quote(&new.name),
                literal(value)
            ),
            "",
        ));
    }
    Ok(ops)
}

fn column_changes(
    table: &Table,
    old: &Column,
    new: &Column,
    ty: &dyn Fn(&str) -> String,
) -> Result<Vec<Op>, String> {
    let t = quote(&table.name);
    let c = quote(&new.name);
    let mut ops = Vec::new();
    if old.generated != new.generated || old.identity != new.identity {
        return Err(format!(
            "`{}.{}` changed its GENERATED / IDENTITY definition; write that change as a hand-made migration",
            table.name, new.name
        ));
    }
    if old.sql_type != new.sql_type {
        ops.push(
            Op::new(
                format!(
                    "ALTER TABLE {t} ALTER COLUMN {c} TYPE {} USING {c}::{}",
                    ty(&new.sql_type),
                    ty(&new.sql_type)
                ),
                format!(
                    "ALTER TABLE {t} ALTER COLUMN {c} TYPE {} USING {c}::{}",
                    ty(&old.sql_type),
                    ty(&old.sql_type)
                ),
            )
            .destructive(format!(
                "changes the type of `{}.{}` from {} to {}",
                table.name, new.name, old.sql_type, new.sql_type
            )),
        );
    }
    if old.default != new.default {
        let set = |d: &Option<String>| match d {
            Some(d) => format!("ALTER TABLE {t} ALTER COLUMN {c} SET DEFAULT {d}"),
            None => format!("ALTER TABLE {t} ALTER COLUMN {c} DROP DEFAULT"),
        };
        ops.push(Op::new(set(&new.default), set(&old.default)));
    }
    if old.nullable != new.nullable {
        let set = format!("ALTER TABLE {t} ALTER COLUMN {c} SET NOT NULL");
        let drop = format!("ALTER TABLE {t} ALTER COLUMN {c} DROP NOT NULL");
        ops.push(if new.nullable {
            Op::new(drop, set)
        } else {
            Op::new(set, drop).destructive(format!(
                "makes `{}.{}` NOT NULL, which fails when it holds NULLs",
                table.name, new.name
            ))
        });
    }
    Ok(ops)
}

fn apply_rename(old: &mut Schema, rename: &Rename) -> Result<Op, String> {
    match rename {
        Rename::Table { from, to } => {
            let schema_prefix = from.rsplit_once('.').map(|(s, _)| format!("{s}."));
            let new_name = format!("{}{to}", schema_prefix.unwrap_or_default());
            let table = old
                .tables
                .iter_mut()
                .find(|t| &t.name == from)
                .ok_or_else(|| format!("--rename: the previous schema has no table `{from}`"))?;
            table.name.clone_from(&new_name);
            for t in &mut old.tables {
                for fk in &mut t.foreign_keys {
                    if &fk.ref_table == from {
                        fk.ref_table.clone_from(&new_name);
                    }
                }
            }
            for index in &mut old.indexes {
                if &index.table == from {
                    index.table.clone_from(&new_name);
                }
            }
            Ok(Op::new(
                format!("ALTER TABLE {} RENAME TO {}", quote(from), quote(to)),
                format!(
                    "ALTER TABLE {} RENAME TO {}",
                    quote(&new_name),
                    quote(naming::unqualified(from))
                ),
            ))
        }
        Rename::Column { table, from, to } => {
            let t = old
                .tables
                .iter_mut()
                .find(|t| &t.name == table)
                .ok_or_else(|| format!("--rename: the previous schema has no table `{table}`"))?;
            let column = t
                .columns
                .iter_mut()
                .find(|c| &c.name == from)
                .ok_or_else(|| format!("--rename: `{table}` has no column `{from}`"))?;
            column.name.clone_from(to);
            let rename = |cols: &mut Vec<String>| {
                for c in cols.iter_mut().filter(|c| *c == from) {
                    c.clone_from(to);
                }
            };
            rename(&mut t.primary_key);
            t.uniques.iter_mut().for_each(|u| rename(&mut u.columns));
            t.foreign_keys
                .iter_mut()
                .for_each(|fk| rename(&mut fk.columns));
            let table_name = t.name.clone();
            for other in &mut old.tables {
                for fk in other
                    .foreign_keys
                    .iter_mut()
                    .filter(|fk| fk.ref_table == table_name)
                {
                    rename(&mut fk.ref_columns);
                }
            }
            Ok(Op::new(
                format!(
                    "ALTER TABLE {} RENAME COLUMN {} TO {}",
                    quote(table),
                    quote(from),
                    quote(to)
                ),
                format!(
                    "ALTER TABLE {} RENAME COLUMN {} TO {}",
                    quote(table),
                    quote(to),
                    quote(from)
                ),
            ))
        }
    }
}

pub(crate) fn create_enum(e: &EnumType) -> String {
    let values: Vec<String> = e.values.iter().map(|v| literal(v)).collect();
    format!(
        "CREATE TYPE {} AS ENUM ({})",
        quote(&e.name),
        values.join(", ")
    )
}

/// `CREATE TABLE` without foreign keys (they are added separately).
pub(crate) fn create_table(table: &Table, ty: &dyn Fn(&str) -> String) -> String {
    let mut lines: Vec<String> = table.columns.iter().map(|c| column_def(c, ty)).collect();
    if !table.primary_key.is_empty() {
        lines.push(format!(
            "CONSTRAINT {} PRIMARY KEY ({})",
            quote(&pkey(table)),
            columns(&table.primary_key)
        ));
    }
    for u in &table.uniques {
        lines.push(format!(
            "CONSTRAINT {} UNIQUE ({})",
            quote(&u.name),
            columns(&u.columns)
        ));
    }
    for c in &table.checks {
        lines.push(format!("CONSTRAINT {} CHECK ({})", quote(&c.name), c.expr));
    }
    format!(
        "CREATE TABLE {} (\n    {}\n)",
        quote(&table.name),
        lines.join(",\n    ")
    )
}

fn column_def(c: &Column, ty: &dyn Fn(&str) -> String) -> String {
    let mut def = format!("{} {}", quote(&c.name), ty(&c.sql_type));
    if let Some(expr) = &c.generated {
        def.push_str(&format!(" GENERATED ALWAYS AS ({expr}) STORED"));
    }
    if let Some(kind) = &c.identity {
        def.push_str(&format!(" GENERATED {kind} AS IDENTITY"));
    }
    if !c.nullable {
        def.push_str(" NOT NULL");
    }
    if let Some(d) = &c.default {
        def.push_str(&format!(" DEFAULT {d}"));
    }
    def
}

fn add_fk(table: &Table, fk: &ForeignKey) -> Op {
    let t = quote(&table.name);
    let mut up = format!(
        "ALTER TABLE {t} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
        quote(&fk.name),
        columns(&fk.columns),
        quote(&fk.ref_table),
        columns(&fk.ref_columns)
    );
    if let Some(action) = &fk.on_delete {
        up.push_str(&format!(" ON DELETE {action}"));
    }
    if let Some(action) = &fk.on_update {
        up.push_str(&format!(" ON UPDATE {action}"));
    }
    Op::new(
        up,
        format!("ALTER TABLE {t} DROP CONSTRAINT {}", quote(&fk.name)),
    )
}

fn add_unique(table: &Table, u: &Unique) -> Op {
    let t = quote(&table.name);
    Op::new(
        format!(
            "ALTER TABLE {t} ADD CONSTRAINT {} UNIQUE ({})",
            quote(&u.name),
            columns(&u.columns)
        ),
        format!("ALTER TABLE {t} DROP CONSTRAINT {}", quote(&u.name)),
    )
}

fn add_check(table: &Table, c: &Check) -> Op {
    let t = quote(&table.name);
    Op::new(
        format!(
            "ALTER TABLE {t} ADD CONSTRAINT {} CHECK ({})",
            quote(&c.name),
            c.expr
        ),
        format!("ALTER TABLE {t} DROP CONSTRAINT {}", quote(&c.name)),
    )
}

fn pkey(table: &Table) -> String {
    format!("{}_pkey", naming::unqualified(&table.name))
}

/// An index lives in its table's schema.
fn index_name(table: &str, index: &str) -> String {
    match table.rsplit_once('.') {
        Some((schema, _)) if !index.contains('.') => format!("{schema}.{index}"),
        _ => index.to_owned(),
    }
}

fn columns(cols: &[String]) -> String {
    cols.iter().map(|c| quote(c)).collect::<Vec<_>>().join(", ")
}

fn render_type(sql_type: &str, enums: &[&str]) -> String {
    let (base, dims) = match sql_type.find("[]") {
        Some(i) => (&sql_type[..i], &sql_type[i..]),
        None => (sql_type, ""),
    };
    if enums.contains(&base) {
        format!("{}{dims}", quote(base))
    } else {
        sql_type.to_owned()
    }
}

/// A SQL string literal.
fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Join steps into one migration's `up` and `down` scripts. `down` is empty
/// when a step can't be undone.
pub(crate) fn scripts(ops: &[Op]) -> (String, String) {
    let up = ops
        .iter()
        .map(|op| format!("{};\n", op.up))
        .collect::<String>();
    let down = if ops.iter().any(|op| op.down.trim().is_empty()) {
        String::new()
    } else {
        ops.iter()
            .rev()
            .map(|op| format!("{};\n", op.down))
            .collect()
    };
    (up, down)
}

#[cfg(test)]
mod tests;
