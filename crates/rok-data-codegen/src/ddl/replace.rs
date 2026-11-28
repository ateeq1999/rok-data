//! A changed function or trigger is replaced in place (`CREATE OR REPLACE`)
//! rather than dropped and created again: the objects that depend on it
//! (a function's triggers) stay where they are.

use sqlparser::ast::{ArgMode, Statement};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::ir::Schema;
use crate::parse::object_name;

use super::Op;

/// What a statement creates, when it can be replaced in place.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Replaceable {
    /// `CREATE FUNCTION name(args)`: one function per name and argument
    /// types.
    Function { name: String, args: Vec<String> },
    /// `CREATE TRIGGER name … ON table`: one trigger per name and table.
    /// A constraint trigger can't be replaced (PostgreSQL refuses `OR
    /// REPLACE` on one), so it is none of these.
    Trigger { name: String, table: String },
}

/// The one statement `sql` holds, parsed.
fn statement(sql: &str) -> Option<Statement> {
    let mut statements = Parser::parse_sql(&PostgreSqlDialect {}, sql).ok()?;
    (statements.len() == 1).then(|| statements.remove(0))
}

impl Replaceable {
    /// The function or trigger `sql` creates, if it is one.
    fn of(sql: &str) -> Option<Self> {
        match statement(sql)? {
            Statement::CreateFunction(function) => Some(Self::Function {
                name: object_name(&function.name),
                // `OUT` arguments are not part of a function's signature.
                args: function
                    .args
                    .unwrap_or_default()
                    .iter()
                    .filter(|arg| !matches!(arg.mode, Some(ArgMode::Out)))
                    .map(|arg| arg.data_type.to_string().to_lowercase())
                    .collect(),
            }),
            Statement::CreateTrigger(trigger) if !trigger.is_constraint => Some(Self::Trigger {
                name: object_name(&trigger.name),
                table: object_name(&trigger.table_name),
            }),
            _ => None,
        }
    }
}

/// The extras of `old` and `new` that create the same object under
/// changed text, as `(old index, new index)`.
///
/// `Err` when a function is gone and one of its name takes other argument
/// types: PostgreSQL can't replace a function across a change of
/// signature, and dropping it would take its triggers along.
pub(super) fn pairs(old: &Schema, new: &Schema) -> Result<Vec<(usize, usize)>, String> {
    let (before, after) = (created(old), created(new));
    let has =
        |list: &[(usize, Replaceable)], what: &Replaceable| list.iter().any(|(_, w)| w == what);
    // A function that is gone, and a function of its name that is new.
    let gone = before.iter().filter(|(_, was)| !has(&after, was));
    for (_, was) in gone {
        let Replaceable::Function { name, args } = was else {
            continue;
        };
        let changed = after.iter().find_map(|(_, now)| match now {
            Replaceable::Function { name: same, args } if same == name && !has(&before, now) => {
                Some(args)
            }
            _ => None,
        });
        if let Some(now) = changed {
            return Err(signature_changed(name, args, now, old, new));
        }
    }
    Ok(after
        .iter()
        .filter_map(|(n, what)| {
            let (o, _) = before.iter().find(|(_, was)| was == what)?;
            (old.extras[*o].up != new.extras[*n].up).then_some((*o, *n))
        })
        .collect())
}

/// The refusal of a function whose argument types changed, naming the
/// triggers that use it.
fn signature_changed(
    name: &str,
    was: &[String],
    now: &[String],
    old: &Schema,
    new: &Schema,
) -> String {
    let mut triggers = triggers_of(name, old);
    for trigger in triggers_of(name, new) {
        if !triggers.contains(&trigger) {
            triggers.push(trigger);
        }
    }
    let used_by = if triggers.is_empty() {
        String::new()
    } else {
        format!(
            ", and the triggers using it ({}) with it",
            triggers.join(", ")
        )
    };
    format!(
        "function `{name}({})` changed its signature to `{name}({})`; PostgreSQL can't \
         replace a function whose argument types change, and dropping it drops what uses \
         it{used_by}. Write that change as a hand-made migration",
        was.join(", "),
        now.join(", ")
    )
}

/// The triggers in `schema` that execute function `name`, as
/// `trigger on table`.
fn triggers_of(name: &str, schema: &Schema) -> Vec<String> {
    schema
        .extras
        .iter()
        .filter_map(|extra| match statement(&extra.up)? {
            Statement::CreateTrigger(trigger)
                if trigger
                    .exec_body
                    .as_ref()
                    .is_some_and(|body| object_name(&body.func_desc.name) == name) =>
            {
                Some(format!(
                    "`{}` on `{}`",
                    object_name(&trigger.name),
                    object_name(&trigger.table_name)
                ))
            }
            _ => None,
        })
        .collect()
}

/// The replaceable objects a schema's extras create, by extra index.
fn created(schema: &Schema) -> Vec<(usize, Replaceable)> {
    schema
        .extras
        .iter()
        .enumerate()
        .filter(|(_, extra)| !extra.before_tables)
        .filter_map(|(i, extra)| Replaceable::of(&extra.up).map(|what| (i, what)))
        .collect()
}

/// The step that replaces `old`'s text with `new`'s, and back.
pub(super) fn replace(old: &str, new: &str) -> Op {
    Op::new(or_replace(new), or_replace(old))
}

/// `CREATE …` as `CREATE OR REPLACE …`; a statement that already says
/// `OR REPLACE` is kept as written.
fn or_replace(sql: &str) -> String {
    let sql = sql.trim_start();
    let starts_with_create = sql
        .get(..6)
        .is_some_and(|word| word.eq_ignore_ascii_case("create"));
    let rest = sql.get(6..).unwrap_or_default().trim_start();
    let says_or = rest
        .get(..3)
        .is_some_and(|word| word.eq_ignore_ascii_case("or "));
    if !starts_with_create || says_or {
        return sql.to_owned();
    }
    format!("CREATE OR REPLACE {rest}")
}

#[cfg(test)]
mod tests;
