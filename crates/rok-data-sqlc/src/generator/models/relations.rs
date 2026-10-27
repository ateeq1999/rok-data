//! The row's relation attributes: the schema's foreign keys become
//! `#[rok(belongs_to …)]` on the holding field and `#[rok(has_many …)]` /
//! `#[rok(has_one …)]` on the referenced table. The derive expands them into
//! the relation constants and the `Select` accessors (spec D6), so the
//! generated code states the relationship — it does not re-derive it here,
//! and the macro stays the one place that knows the expansion.

use crate::schema::{RelationshipKind, Table};

/// One belongs-to: the attribute a foreign-key field carries.
struct BelongsTo {
    /// The foreign-key column's SQL name; this table's field joins on it.
    sql_name: String,
    /// The attribute line, without indentation.
    attr: String,
}

/// The relations of one table, resolved against the tables the attributes
/// name: the belongs-to side per foreign-key column, the has-many and
/// has-one sides per recorded relationship, and the sibling-model imports
/// both need. A many-to-many (`through` a join table) becomes
/// `#[rok(belongs_to_many(…))]` when the join table pairs the two tables'
/// single-column primary keys, the shape the runtime's pivot verbs write.
pub(super) struct Relations {
    belongs_to: Vec<BelongsTo>,
    /// The container attribute lines, in recorded order.
    container: Vec<String>,
    imports: Vec<String>,
}

impl Relations {
    /// Reads `table.relationships` — the name and kind the schema's
    /// foreign-key inference records once — and resolves each side against
    /// `tables`, which carries the struct and column constants the
    /// attributes quote.
    pub(super) fn collect(table: &Table, tables: &[Table]) -> Self {
        let mut belongs_to = Vec::new();
        let mut container = Vec::new();
        let mut imports = std::collections::BTreeSet::new();
        for relationship in table.relationships.iter().filter(|relationship| {
            relationship.through.is_none() || relationship.kind == RelationshipKind::ManyToMany
        }) {
            // The attr names no skippable table: the schema only records
            // relationships among the tables generated together.
            let Some(other) = tables
                .iter()
                .find(|other| other.module_name == relationship.target)
            else {
                continue;
            };
            // A self-reference names the row's own struct, which this file
            // already defines.
            let sibling = (other.module_name != table.module_name).then(|| {
                format!(
                    "super::super::{}::models::{}",
                    other.module_name, other.struct_name
                )
            });
            match relationship.kind {
                RelationshipKind::BelongsTo => {
                    let derived = derived_name(&relationship.column, table)
                        .filter(|derived| *derived == relationship.name);
                    let attr = match derived {
                        // The derive names the relation off the field: the
                        // attribute names nothing redundant.
                        Some(_) => format!("#[rok(belongs_to = {})]", other.struct_name),
                        // Any other edge (`currency_code` → `currency`, a
                        // column named like the referenced row, a
                        // multi-FK edge) states the name explicitly.
                        None => format!(
                            "#[rok(belongs_to({} = {}))]",
                            relationship.name, other.struct_name
                        ),
                    };
                    imports.extend(sibling.clone());
                    belongs_to.push(BelongsTo {
                        sql_name: relationship.column.clone(),
                        attr,
                    });
                }
                RelationshipKind::HasMany | RelationshipKind::HasOne => {
                    // The value is the child's column constant, which the
                    // child's own derive writes: the foreign-key field
                    // there, by its unraw name uppercased.
                    let Some(foreign_key) = other
                        .columns
                        .iter()
                        .find(|column| column.sql_name == relationship.target_column)
                    else {
                        continue;
                    };
                    let kind = match relationship.kind {
                        RelationshipKind::HasMany => "has_many",
                        _ => "has_one",
                    };
                    imports.extend(sibling);
                    container.push(format!(
                        "#[rok({}({} = {}::{}))]",
                        kind,
                        relationship.name,
                        other.struct_name,
                        foreign_key.rust_name.to_uppercase()
                    ));
                }
                RelationshipKind::ManyToMany => {
                    if let Some(attr) = many_to_many(table, other, relationship, tables) {
                        imports.extend(sibling);
                        container.push(attr);
                    }
                }
            }
        }
        Relations {
            belongs_to,
            container,
            imports: imports.into_iter().collect(),
        }
    }

    /// The field attribute a column carries, when a belongs-to joins on it.
    pub(super) fn field_attr(&self, sql_name: &str) -> Option<&str> {
        self.belongs_to
            .iter()
            .find(|belongs_to| belongs_to.sql_name == sql_name)
            .map(|belongs_to| belongs_to.attr.as_str())
    }

    /// The container attribute lines of the row's derive.
    pub(super) fn container_attrs(&self) -> &[String] {
        &self.container
    }

    /// The sibling models modules the attributes reference.
    pub(super) fn imports(&self) -> &[String] {
        &self.imports
    }
}

/// What the derive would name a bare `#[rok(belongs_to = Target)]` for the
/// foreign-key column: the field without its `_id` suffix. `Some(_)` means
/// the attribute may leave the naming to the derive; `None` — or a recorded
/// name that differs — means the edge states its name itself.
fn derived_name<'a>(sql_name: &str, table: &'a Table) -> Option<&'a str> {
    let column = table
        .columns
        .iter()
        .find(|column| column.sql_name == sql_name)?;
    let name = column.rust_name.strip_suffix("_id")?;
    (!name.is_empty()).then_some(name)
}

/// `#[rok(belongs_to_many(name = Target, pivot = "join_table", foreign_key
/// = "…", related_key = "…"))]` for a many-to-many edge, or `None` when the
/// join table doesn't pair the two tables' single-column primary keys (the
/// runtime's pivot verbs key on those).
fn many_to_many(
    table: &Table,
    other: &Table,
    relationship: &crate::schema::Relationship,
    tables: &[Table],
) -> Option<String> {
    let through = relationship.through.as_ref()?;
    let keyed = |t: &Table, column: &str| t.primary_key.len() == 1 && t.primary_key[0] == column;
    if !keyed(table, &relationship.column) || !keyed(other, &relationship.target_column) {
        return None;
    }
    let pivot = tables
        .iter()
        .find(|candidate| candidate.module_name == through.table)?;
    let pivot = match &pivot.schema {
        Some(schema) => format!("{schema}.{}", pivot.sql_name),
        None => pivot.sql_name.clone(),
    };
    Some(format!(
        "#[rok(belongs_to_many({} = {}, pivot = \"{pivot}\", foreign_key = \"{}\", related_key = \"{}\"))]",
        relationship.name, other.struct_name, through.column, through.target_column
    ))
}
