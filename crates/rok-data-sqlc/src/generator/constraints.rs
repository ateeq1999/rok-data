//! `db::constraints`: the table of the schema's `-- rok:constraint`
//! annotations, by constraint name, so an API answers a violation with
//! its field and message instead of PostgreSQL's constraint name.

use crate::schema::ConstraintMessage;

use super::HEADER;

/// The module's name under the output directory.
pub const MODULE: &str = "constraints";

/// The items every table has: the kind, the entry type, the lookups.
const ITEMS: &str = r#"
/// What a constraint is, which decides what a violation answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstraintKind {
    /// A `CHECK`: the value is wrong (422 on the field).
    Check,
    /// A primary key, unique constraint or unique index: the value is taken
    /// (409).
    Unique,
    /// A foreign key: the change breaks a reference (409).
    ForeignKey,
    /// A name a trigger raises (`RAISE … USING CONSTRAINT`): the value is
    /// wrong (422 on the field).
    Raised,
}

/// An annotated constraint: what to tell the caller who breaks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Constraint {
    /// The constraint's name, as PostgreSQL reports a violation of it.
    pub name: &'static str,
    /// The constrained table; `None` for a name a trigger raises.
    pub table: Option<&'static str>,
    /// The field a violation is about, when the annotation names one.
    pub field: Option<&'static str>,
    /// The message the annotation gives.
    pub message: &'static str,
    /// What the constraint is.
    pub kind: ConstraintKind,
}

/// The annotated constraint named `name`, if there is one.
#[must_use]
pub fn lookup(name: &str) -> Option<&'static Constraint> {
    CONSTRAINTS
        .binary_search_by(|constraint| constraint.name.cmp(name))
        .ok()
        .map(|index| &CONSTRAINTS[index])
}

/// The annotated constraint `error` violates, if there is one.
#[must_use]
pub fn of_error(error: &rok_data::Error) -> Option<&'static Constraint> {
    lookup(error.constraint()?)
}
"#;

/// Renders `constraints.rs`, or `None` when the schema annotates none.
/// `messages` are sorted by name, as `lookup`'s binary search needs.
pub fn render(messages: &[ConstraintMessage]) -> Option<String> {
    if messages.is_empty() {
        return None;
    }
    let option = |value: Option<&String>| match value {
        Some(value) => format!("Some({value:?})"),
        None => "None".to_owned(),
    };
    let mut output = format!("{HEADER}\n");
    output.push_str(
        "//! The schema's `-- rok:constraint` annotations: the field and message a\n\
         //! violation of each constraint answers with.\n",
    );
    output.push_str(ITEMS);
    output.push_str(
        "\n/// Every annotated constraint, sorted by name.\npub const CONSTRAINTS: &[Constraint] = &[\n",
    );
    for message in messages {
        output.push_str(&format!(
            "    Constraint {{\n        name: {:?},\n        table: {},\n        field: {},\n        \
             message: {:?},\n        kind: ConstraintKind::{},\n    }},\n",
            message.name,
            option(message.table.as_ref()),
            option(message.field.as_ref()),
            message.message,
            message.kind.variant(),
        ));
    }
    output.push_str("];\n");
    Some(output)
}
