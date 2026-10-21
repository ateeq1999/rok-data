//! `ALTER TABLE` applied to the parsed schema.
//!
//! The truth may carry `ALTER TABLE` statements beside the tables they
//! change (ADR 0017 in the engine's series). The staging diff works on the
//! parsed tables only, so an `ALTER TABLE` this model can represent is
//! applied to the tables themselves: the change rides no `Extra` step, and
//! folding such a change into the truth's `CREATE TABLE` later stages no
//! redundant drop-and-add pair. A clause the model cannot represent falls
//! back to the extras path, where `-- rok:down` supplies the statement's
//! undo and the statement rides as written.
//!
//! This mirrors the engine's own `parser/postgres/alter.rs` on the engine's
//! richer table model. The column and constraint builders below are the
//! ones `CREATE TABLE` parsing uses, so both statement kinds build columns
//! and constraints the same way.
//!
//! The collaterals a live `ALTER TABLE` drags along are kept in their
//! narrowest form: a rename follows the columns into every key that named
//! them (comments ride no rename — the engine's wording, unchanged), and a
//! dropped key column takes its keys with it, as PostgreSQL does.

use sqlparser::ast::{
    AlterColumnOperation, AlterTableOperation, ColumnDef, GeneratedAs, ObjectName, TableConstraint,
};

use crate::ir::{Check, Column, ForeignKey, Schema, Table, Unique};
use crate::naming;
use crate::parse::check::{constraint_name, expr_ident, ident, object_name};
use crate::types;

/// One column definition built from its `ColumnDef`: the `Column`, and the
/// constraints its column-level options imply, named after the table.
pub(crate) struct BuiltColumn {
    /// The column as the options leave it.
    column: Column,
    /// Whether the definition carries a column-level `PRIMARY KEY`.
    primary_key: bool,
    /// A column-level `UNIQUE` and the constraint name it takes.
    unique: Option<Unique>,
    /// A column-level `REFERENCES` and its constraint name.
    foreign_key: Option<ForeignKey>,
    /// A column-level `CHECK` and its constraint name.
    check: Option<Check>,
}

/// Builds one column definition: the `Column` per its options, and the
/// constraints its options imply, named after `table`. Errors are the
/// plain reasons each caller weaves into its own message.
///
/// The `notes` are the file's column comments, ((column, doc, tenant));
/// `CREATE TABLE` parsing passes the file's, `ALTER TABLE` passes none —
/// an `ADD COLUMN`'s comments would be read as a column's by the same
/// reader.
pub(crate) fn build_column(
    table: &Table,
    def: &ColumnDef,
    notes: &[(String, Vec<String>, bool)],
) -> Result<BuiltColumn, String> {
    let column_name = ident(&def.name);
    let short = naming::unqualified(&table.name).to_owned();
    let column = Column {
        name: column_name.clone(),
        sql_type: types::canonical(&def.data_type.to_string()),
        nullable: true,
        default: None,
        generated: None,
        identity: None,
        tenant: false,
        doc: Vec::new(),
    };
    let mut built = BuiltColumn {
        column,
        primary_key: false,
        unique: None,
        foreign_key: None,
        check: None,
    };
    for option in &def.options {
        match &option.option {
            sqlparser::ast::ColumnOption::NotNull => built.column.nullable = false,
            sqlparser::ast::ColumnOption::Null => built.column.nullable = true,
            sqlparser::ast::ColumnOption::Default(expr) => {
                built.column.default = Some(expr.to_string());
            }
            sqlparser::ast::ColumnOption::PrimaryKey(_) => {
                if !table.primary_key.is_empty() {
                    return Err(format!("table `{}` has two primary keys", table.name));
                }
                built.primary_key = true;
            }
            sqlparser::ast::ColumnOption::Unique(_) => {
                built.unique = Some(Unique {
                    name: format!("{short}_{column_name}_key"),
                    columns: vec![column_name.clone()],
                });
            }
            sqlparser::ast::ColumnOption::ForeignKey(fk) => {
                built.foreign_key = Some(ForeignKey {
                    name: constraint_name(fk.name.as_ref(), || {
                        format!("{short}_{column_name}_fkey")
                    }),
                    columns: vec![column_name.clone()],
                    ref_table: object_name(&fk.foreign_table),
                    ref_columns: fk.referred_columns.iter().map(ident).collect(),
                    on_delete: fk.on_delete.as_ref().map(ToString::to_string),
                    on_update: fk.on_update.as_ref().map(ToString::to_string),
                });
            }
            sqlparser::ast::ColumnOption::Check(check) => {
                built.check = Some(Check {
                    name: constraint_name(check.name.as_ref(), || {
                        format!("{short}_{column_name}_check")
                    }),
                    expr: check.expr.to_string(),
                });
            }
            sqlparser::ast::ColumnOption::Generated {
                generated_as,
                generation_expr,
                ..
            } => match generation_expr {
                Some(expr) => built.column.generated = Some(expr.to_string()),
                None => {
                    built.column.identity = Some(
                        match generated_as {
                            GeneratedAs::Always => "ALWAYS",
                            _ => "BY DEFAULT",
                        }
                        .to_owned(),
                    );
                }
            },
            other => {
                return Err(format!(
                    "column option `{other}` on `{column_name}` is not supported"
                ));
            }
        }
    }
    if let Some((doc, tenant)) = notes
        .iter()
        .find_map(|(c, d, t)| (c.eq_ignore_ascii_case(&column_name)).then(|| (d.clone(), *t)))
    {
        built.column.doc = doc;
        built.column.tenant = tenant;
    }
    Ok(built)
}

/// Puts a built column into `table`: the column, the constraints its
/// options imply, and the NOT NULL the column-level primary key carries.
pub(crate) fn place_column(table: &mut Table, built: BuiltColumn) {
    let mut built = built;
    let name = built.column.name.clone();
    if built.primary_key {
        table.primary_key = vec![name.clone()];
        built.column.nullable = false;
    }
    if let Some(unique) = built.unique {
        table.uniques.push(unique);
    }
    if let Some(foreign_key) = built.foreign_key {
        table.foreign_keys.push(foreign_key);
    }
    if let Some(check) = built.check {
        table.checks.push(check);
    }
    table.columns.push(built.column);
}

/// Adds one table-level constraint (`CONSTRAINT …`) to `table`. The
/// counter names the unnamed checks, `books_check`, `books_check1`, ….
pub(crate) fn add_table_constraint(
    table: &mut Table,
    constraint: &TableConstraint,
    unnamed_checks: &mut usize,
) -> Result<(), String> {
    let short = naming::unqualified(&table.name).to_owned();
    match constraint {
        TableConstraint::PrimaryKey(pk) => {
            if !table.primary_key.is_empty() {
                return Err(format!("table `{}` has two primary keys", table.name));
            }
            table.primary_key = pk
                .columns
                .iter()
                .map(|c| expr_ident(&c.column.expr))
                .collect();
        }
        TableConstraint::Unique(u) => {
            let columns: Vec<String> = u
                .columns
                .iter()
                .map(|c| expr_ident(&c.column.expr))
                .collect();
            let name = constraint_name(u.name.as_ref(), || {
                format!("{short}_{}_key", columns.join("_"))
            });
            table.uniques.push(Unique { name, columns });
        }
        TableConstraint::ForeignKey(fk) => {
            let columns: Vec<String> = fk.columns.iter().map(ident).collect();
            let name = constraint_name(fk.name.as_ref(), || {
                format!("{short}_{}_fkey", columns.join("_"))
            });
            table.foreign_keys.push(ForeignKey {
                name,
                columns,
                ref_table: object_name(&fk.foreign_table),
                ref_columns: fk.referred_columns.iter().map(ident).collect(),
                on_delete: fk.on_delete.as_ref().map(ToString::to_string),
                on_update: fk.on_update.as_ref().map(ToString::to_string),
            });
        }
        TableConstraint::Check(check) => {
            let name = constraint_name(check.name.as_ref(), || {
                *unnamed_checks += 1;
                match *unnamed_checks {
                    1 => format!("{short}_check"),
                    n => format!("{short}_check{}", n - 1),
                }
            });
            table.checks.push(Check {
                name,
                expr: check.expr.to_string(),
            });
        }
        other => {
            return Err(format!("table constraint `{other}` is not supported"));
        }
    }
    Ok(())
}

/// Applies an `ALTER TABLE` to the schema's parsed tables, leaving them as
/// the statement leaves the real schema. `Ok` when every clause was
/// applied: the diff derives the change from the tables, and the
/// statement rides no `Extra`. `Err` when a clause is not representable in
/// this model (or the table is not declared yet): the caller falls back,
/// and the statement rides as written.
pub(crate) fn apply(
    schema: &mut Schema,
    name: &ObjectName,
    operations: &[AlterTableOperation],
) -> Result<(), String> {
    let target = object_name(name);
    let Some(index) = find_table(schema, &target) else {
        return Err(format!(
            "ALTER TABLE: `{target}` is not declared by the tables parsed so far"
        ));
    };
    let mut candidate = schema.tables[index].clone();
    apply_operations(&mut candidate, operations)?;
    // A primary key's columns are NOT NULL, however it was added.
    let key = candidate.primary_key.clone();
    for column in &mut candidate.columns {
        if key.contains(&column.name) {
            column.nullable = false;
        }
    }
    schema.tables[index] = candidate;
    Ok(())
}

/// The table the statement names: by its full name, or — unambiguously —
/// by bare name, so a truth that qualifies its tables accepts an
/// unqualified `ALTER TABLE` on the default schema's one.
fn find_table(schema: &Schema, name: &str) -> Option<usize> {
    let exact = schema.tables.iter().position(|t| t.name == name);
    if exact.is_some() {
        return exact;
    }
    let leaf = naming::unqualified(name);
    let matches: Vec<usize> = schema
        .tables
        .iter()
        .enumerate()
        .filter(|(_, t)| naming::unqualified(&t.name) == leaf)
        .map(|(index, _)| index)
        .collect();
    (matches.len() == 1).then(|| matches[0])
}

/// Applies the clauses in order; one error stops the lot, and the table is
/// left as it was.
fn apply_operations(table: &mut Table, operations: &[AlterTableOperation]) -> Result<(), String> {
    let mut unnamed_checks = 0;
    for operation in operations {
        match operation {
            AlterTableOperation::AddColumn {
                if_not_exists,
                column_def,
                ..
            } => add_column(table, column_def, *if_not_exists)?,
            AlterTableOperation::DropColumn {
                column_names,
                if_exists,
                ..
            } => {
                for name in column_names {
                    drop_column(table, &ident(name), *if_exists)?;
                }
            }
            AlterTableOperation::RenameColumn {
                old_column_name,
                new_column_name,
            } => rename_column(table, &ident(old_column_name), &ident(new_column_name))?,
            AlterTableOperation::AddConstraint { constraint, .. } => {
                add_table_constraint(table, constraint, &mut unnamed_checks)?;
            }
            AlterTableOperation::AlterColumn { column_name, op } => {
                alter_column(table, &ident(column_name), op)?;
            }
            AlterTableOperation::RenameConstraint { old_name, new_name } => {
                // Only the unique constraints carry tracked names; the
                // engine's rule, unchanged.
                let old_name = ident(old_name);
                if let Some(unique) = table
                    .uniques
                    .iter_mut()
                    .find(|unique| unique.name == old_name)
                {
                    unique.name = ident(new_name);
                }
            }
            // Clauses that leave the columns and keys as they are. Triggers
            // are followed by `CREATE` and `DROP TRIGGER` only.
            AlterTableOperation::EnableRowLevelSecurity
            | AlterTableOperation::DisableRowLevelSecurity
            | AlterTableOperation::SetOptionsParens { .. }
            | AlterTableOperation::ReplicaIdentity { .. }
            | AlterTableOperation::OwnerTo { .. }
            | AlterTableOperation::ValidateConstraint { .. }
            | AlterTableOperation::EnableTrigger { .. }
            | AlterTableOperation::DisableTrigger { .. }
            | AlterTableOperation::EnableAlwaysTrigger { .. }
            | AlterTableOperation::EnableReplicaTrigger { .. }
            | AlterTableOperation::EnableRule { .. }
            | AlterTableOperation::DisableRule { .. }
            | AlterTableOperation::EnableAlwaysRule { .. }
            | AlterTableOperation::EnableReplicaRule { .. } => {}
            other => {
                return Err(format!("unsupported ALTER TABLE clause `{other}`"));
            }
        }
    }
    Ok(())
}

fn add_column(table: &mut Table, def: &ColumnDef, if_not_exists: bool) -> Result<(), String> {
    let name = ident(&def.name);
    if table.column(&name).is_some() {
        if if_not_exists {
            return Ok(());
        }
        return Err(format!("ADD COLUMN: column `{name}` already exists"));
    }
    let built = build_column(table, def, &[])?;
    place_column(table, built);
    Ok(())
}

/// Drops a column. Like PostgreSQL, dropping a key column drops the
/// primary key, unique keys, and foreign keys it belongs to.
fn drop_column(table: &mut Table, name: &str, if_exists: bool) -> Result<(), String> {
    let Some(index) = table.columns.iter().position(|column| column.name == name) else {
        if if_exists {
            return Ok(());
        }
        return Err(format!("DROP COLUMN: column `{name}` does not exist"));
    };
    table.columns.remove(index);
    if table.primary_key.iter().any(|key| key == name) {
        table.primary_key.clear();
    }
    let drops = |columns: &[String]| columns.iter().any(|column| column == name);
    table.uniques.retain(|key| !drops(&key.columns));
    table.foreign_keys.retain(|key| !drops(&key.columns));
    Ok(())
}

fn rename_column(table: &mut Table, old: &str, new: &str) -> Result<(), String> {
    let Some(index) = table.columns.iter().position(|column| column.name == old) else {
        return Err(format!("RENAME COLUMN: column `{old}` does not exist"));
    };
    if table.column(new).is_some() {
        return Err(format!("RENAME COLUMN: column `{new}` already exists"));
    }
    table.columns[index].name = new.to_owned();
    let rename = |columns: &mut Vec<String>| {
        for column in columns.iter_mut().filter(|column| *column == old) {
            *column = new.to_owned();
        }
    };
    rename(&mut table.primary_key);
    table
        .uniques
        .iter_mut()
        .for_each(|key| rename(&mut key.columns));
    table
        .foreign_keys
        .iter_mut()
        .for_each(|key| rename(&mut key.columns));
    Ok(())
}

/// Applies an `ALTER COLUMN` change, leaving the column in its final
/// shape. Keys, unique keys, and foreign keys name their columns, so they
/// follow a type change; a generated column's row of machinery is left
/// where it was.
fn alter_column(
    table: &mut Table,
    name: &str,
    operation: &AlterColumnOperation,
) -> Result<(), String> {
    let index = table
        .columns
        .iter()
        .position(|column| column.name == name)
        .ok_or_else(|| format!("ALTER COLUMN: column `{name}` does not exist"))?;
    let is_key = table.primary_key.iter().any(|key| key == name);
    let column_fail = |reason: String| format!("ALTER COLUMN: column `{name}`: {reason}");
    let column = &mut table.columns[index];
    match operation {
        AlterColumnOperation::SetDataType { data_type, .. } => {
            let sql_type = types::canonical(&data_type.to_string());
            if types::is_serial(&sql_type) {
                return Err(column_fail(format!(
                    "type `{data_type}` exists only in CREATE TABLE and ADD COLUMN"
                )));
            }
            column.sql_type = sql_type;
        }
        AlterColumnOperation::SetNotNull => column.nullable = false,
        AlterColumnOperation::DropNotNull => {
            if is_key {
                return Err(format!(
                    "ALTER COLUMN: column `{name}` is in the primary key, so it cannot drop NOT NULL"
                ));
            }
            column.nullable = true;
        }
        AlterColumnOperation::SetDefault { value } => {
            column.default = Some(value.to_string());
        }
        AlterColumnOperation::DropDefault => column.default = None,
        AlterColumnOperation::AddGenerated { generated_as, .. } => {
            if column.nullable {
                return Err(column_fail(
                    "must be NOT NULL before it can become an identity column".to_owned(),
                ));
            }
            column.identity = Some(
                match generated_as {
                    Some(GeneratedAs::Always) => "ALWAYS",
                    _ => "BY DEFAULT",
                }
                .to_owned(),
            );
        }
    }
    Ok(())
}
