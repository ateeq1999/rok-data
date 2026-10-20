//! The renames a user asked for, applied before diffing.

use crate::ir::{Schema, Table};
use crate::parse::quote;

use super::{
    Op, add_check, add_fk, add_unique, apply_rename, column_changes, column_def, columns,
    create_enum, create_table, enum_changes, has_extra, index_name, pkey, render_type,
};

/// A rename the user asked for (`--rename`), applied before diffing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rename {
    /// `old=new` for a table.
    Table {
        /// Previous name (schema-qualified when the table is).
        from: String,
        /// New name, without a schema.
        to: String,
    },
    /// `table.old=new` for a column.
    Column {
        /// The table, by its previous name.
        table: String,
        /// Previous column name.
        from: String,
        /// New column name.
        to: String,
    },
}

impl Rename {
    /// Parse `old=new` or `table.old=new`.
    pub fn parse(spec: &str) -> Option<Self> {
        let (from, to) = spec.split_once('=')?;
        let (from, to) = (from.trim(), to.trim());
        if from.is_empty() || to.is_empty() || to.contains('.') {
            return None;
        }
        Some(match from.rsplit_once('.') {
            Some((table, column)) => Rename::Column {
                table: table.to_owned(),
                from: column.to_owned(),
                to: to.to_owned(),
            },
            None => Rename::Table {
                from: from.to_owned(),
                to: to.to_owned(),
            },
        })
    }
}

/// The steps from `old` to `new`, in an order that works on a live
/// database; their undo runs in reverse.
pub(crate) fn diff(old: &Schema, new: &Schema, renames: &[Rename]) -> Result<Vec<Op>, String> {
    let mut old = old.clone();
    let mut ops = Vec::new();

    // Removed extras go first (views may depend on columns that change).
    for extra in old.extras.iter().rev().filter(|e| !has_extra(new, e)) {
        ops.push(Op::new(&extra.down, &extra.up).destructive_if(
            extra.down.trim().is_empty(),
            "removes a statement that has no down SQL",
        ));
    }
    for extra in new
        .extras
        .iter()
        .filter(|e| e.before_tables && !has_extra(&old, e))
    {
        ops.push(Op::new(&extra.up, &extra.down));
    }

    // Enums.
    for e in &new.enums {
        match old.enums.iter().find(|o| o.name == e.name) {
            None => ops.push(Op::new(
                create_enum(e),
                format!("DROP TYPE {}", quote(&e.name)),
            )),
            Some(o) => ops.extend(enum_changes(o, e)?),
        }
    }

    // Renames.
    for rename in renames {
        ops.push(apply_rename(&mut old, rename)?);
    }

    let enum_names: Vec<&str> = new
        .enums
        .iter()
        .chain(&old.enums)
        .map(|e| e.name.as_str())
        .collect();
    let ty = |sql_type: &str| render_type(sql_type, &enum_names);

    let mut drop_constraints = Vec::new();
    let mut table_changes = Vec::new();
    let mut add_constraints = Vec::new();
    let mut add_fks = Vec::new();

    for table in &new.tables {
        let Some(before) = old.tables.iter().find(|t| t.name == table.name) else {
            table_changes.push(Op::new(
                create_table(table, &ty),
                format!("DROP TABLE {}", quote(&table.name)),
            ));
            for fk in &table.foreign_keys {
                add_fks.push(add_fk(table, fk));
            }
            continue;
        };
        let t = quote(&table.name);
        // Constraints that went away or changed are dropped first.
        if before.primary_key != table.primary_key {
            if !before.primary_key.is_empty() {
                drop_constraints.push(
                    Op::new(
                        format!("ALTER TABLE {t} DROP CONSTRAINT {}", quote(&pkey(before))),
                        format!(
                            "ALTER TABLE {t} ADD CONSTRAINT {} PRIMARY KEY ({})",
                            quote(&pkey(before)),
                            columns(&before.primary_key)
                        ),
                    )
                    .destructive(format!("changes the primary key of `{}`", table.name)),
                );
            }
            if !table.primary_key.is_empty() {
                add_constraints.push(Op::new(
                    format!(
                        "ALTER TABLE {t} ADD CONSTRAINT {} PRIMARY KEY ({})",
                        quote(&pkey(table)),
                        columns(&table.primary_key)
                    ),
                    format!("ALTER TABLE {t} DROP CONSTRAINT {}", quote(&pkey(table))),
                ));
            }
        }
        for fk in before
            .foreign_keys
            .iter()
            .filter(|fk| !table.foreign_keys.contains(fk))
        {
            let mut op = add_fk(before, fk);
            std::mem::swap(&mut op.up, &mut op.down);
            drop_constraints.push(op);
        }
        for fk in table
            .foreign_keys
            .iter()
            .filter(|fk| !before.foreign_keys.contains(fk))
        {
            add_fks.push(add_fk(table, fk));
        }
        for u in before.uniques.iter().filter(|u| !table.uniques.contains(u)) {
            let mut op = add_unique(before, u);
            std::mem::swap(&mut op.up, &mut op.down);
            drop_constraints.push(op);
        }
        for u in table.uniques.iter().filter(|u| !before.uniques.contains(u)) {
            add_constraints.push(add_unique(table, u));
        }
        for c in before.checks.iter().filter(|c| !table.checks.contains(c)) {
            let mut op = add_check(before, c);
            std::mem::swap(&mut op.up, &mut op.down);
            drop_constraints.push(op);
        }
        for c in table.checks.iter().filter(|c| !before.checks.contains(c)) {
            add_constraints.push(add_check(table, c));
        }
        // Columns.
        for column in before
            .columns
            .iter()
            .filter(|c| table.column(&c.name).is_none())
        {
            table_changes.push(
                Op::new(
                    format!("ALTER TABLE {t} DROP COLUMN {}", quote(&column.name)),
                    format!("ALTER TABLE {t} ADD COLUMN {}", column_def(column, &ty)),
                )
                .destructive(format!(
                    "drops column `{}.{}` and its data",
                    table.name, column.name
                )),
            );
        }
        for column in &table.columns {
            match before.column(&column.name) {
                None => {
                    let op = Op::new(
                        format!("ALTER TABLE {t} ADD COLUMN {}", column_def(column, &ty)),
                        format!("ALTER TABLE {t} DROP COLUMN {}", quote(&column.name)),
                    );
                    let needs_value = !column.nullable && !column.has_database_value();
                    table_changes.push(op.destructive_if(
                        needs_value,
                        format!(
                            "adds NOT NULL column `{}.{}` without a DEFAULT, which fails when the table has rows",
                            table.name, column.name
                        ),
                    ));
                }
                Some(old_column) => {
                    table_changes.extend(column_changes(table, old_column, column, &ty)?)
                }
            }
        }
    }

    // Indexes: dropped before columns change, created after.
    let mut drop_indexes = Vec::new();
    let mut create_indexes = Vec::new();
    for index in &old.indexes {
        if !new.indexes.contains(index) {
            drop_indexes.push(Op::new(
                format!(
                    "DROP INDEX IF EXISTS {}",
                    quote(&index_name(&index.table, &index.name))
                ),
                index.sql.clone(),
            ));
        }
    }
    for index in &new.indexes {
        if !old.indexes.contains(index) {
            create_indexes.push(Op::new(
                index.sql.clone(),
                format!(
                    "DROP INDEX IF EXISTS {}",
                    quote(&index_name(&index.table, &index.name))
                ),
            ));
        }
    }

    // Removed tables: their foreign keys first, then the tables.
    let removed: Vec<&Table> = old
        .tables
        .iter()
        .filter(|o| !new.tables.iter().any(|t| t.name == o.name))
        .collect();
    let mut drop_tables = Vec::new();
    for table in &removed {
        for fk in &table.foreign_keys {
            let mut op = add_fk(table, fk);
            std::mem::swap(&mut op.up, &mut op.down);
            drop_constraints.push(op);
        }
    }
    for table in removed.iter().rev() {
        drop_tables.push(
            Op::new(
                format!("DROP TABLE {}", quote(&table.name)),
                create_table(table, &ty),
            )
            .destructive(format!("drops table `{}` and its data", table.name)),
        );
    }

    ops.extend(drop_constraints);
    ops.extend(drop_indexes);
    ops.extend(table_changes);
    ops.extend(add_constraints);
    ops.extend(add_fks);
    ops.extend(create_indexes);
    ops.extend(drop_tables);

    for e in old
        .enums
        .iter()
        .filter(|o| !new.enums.iter().any(|e| e.name == o.name))
    {
        ops.push(
            Op::new(format!("DROP TYPE {}", quote(&e.name)), create_enum(e))
                .destructive(format!("drops enum type `{}`", e.name)),
        );
    }
    for extra in new
        .extras
        .iter()
        .filter(|e| !e.before_tables && !has_extra(&old, e))
    {
        ops.push(Op::new(&extra.up, &extra.down));
    }
    Ok(ops)
}
