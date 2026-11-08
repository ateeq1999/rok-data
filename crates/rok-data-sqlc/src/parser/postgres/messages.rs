//! `-- rok:constraint <name> [<field>] "<message>"`: the field and the
//! message a violation of a constraint maps to, read from any file of the
//! schema inputs and checked against the parsed schema.
//!
//! The name must be a constraint of the schema: a `CHECK`, a primary key
//! or unique constraint, a unique index (`CREATE UNIQUE INDEX <name>`), or
//! a foreign key, under its declared name or the one PostgreSQL gives an
//! unnamed one (`offers_offer_drop_id_fkey`). A trigger can raise any
//! name (`RAISE … USING CONSTRAINT = '<name>'`), which matches no
//! constraint; such a name is accepted when a function body of the schema
//! raises it. The field, when given, must be a column of the constraint's
//! table (of any table, for a raised name, whose table the schema does not
//! say).

use std::collections::BTreeMap;

use super::SchemaSource;
use crate::report::display_path;
use crate::schema::{ConstraintKind, ConstraintMessage, Schema};

/// The annotation's grammar, for the error that it was not followed.
const GRAMMAR: &str = "`-- rok:constraint <name> [<field>] \"<message>\"`";

/// One constraint of the schema a name can be.
struct Known<'a> {
    kind: ConstraintKind,
    /// The table's SQL name and its columns; `None` for a raised name.
    table: Option<(&'a str, Vec<&'a str>)>,
}

/// The `-- rok:constraint` annotations of `sources`, checked against
/// `schema`, sorted by name. Every annotation that names no constraint, or
/// a field that is no column of its table, is an error, all of them
/// listed, each at its `path:line`.
pub fn constraint_messages(
    sources: &[SchemaSource<'_>],
    schema: &Schema,
) -> Result<Vec<ConstraintMessage>, String> {
    let known = known_constraints(sources, schema);
    let mut errors = Vec::new();
    let mut found: BTreeMap<String, (ConstraintMessage, String)> = BTreeMap::new();
    for source in sources {
        for (index, line) in source.sql.lines().enumerate() {
            let Some(annotation) = annotation(line) else {
                continue;
            };
            let place = if source.path.as_os_str().is_empty() {
                format!("line {}", index + 1)
            } else {
                format!("{}:{}", display_path(source.path), index + 1)
            };
            let parsed = parse(annotation)
                .and_then(|(name, field, message)| check(&known, name, field, message));
            match parsed {
                Ok(message) => {
                    if let Some((_, first)) = found.get(&message.name) {
                        errors.push(format!(
                            "{place}: `{}` is annotated with `-- rok:constraint` already, at {first}",
                            message.name
                        ));
                    } else {
                        found.insert(message.name.clone(), (message, place));
                    }
                }
                Err(error) => errors.push(format!("{place}: {error}")),
            }
        }
    }
    if errors.is_empty() {
        Ok(found.into_values().map(|(message, _)| message).collect())
    } else {
        Err(errors.join("\n"))
    }
}

/// The text after `-- rok:constraint` on an annotation line.
fn annotation(line: &str) -> Option<&str> {
    let rest = line
        .trim()
        .strip_prefix("--")?
        .trim_start()
        .strip_prefix("rok:constraint")?;
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then(|| rest.trim())
}

/// The name, the field and the message of an annotation's text.
fn parse(text: &str) -> Result<(&str, Option<&str>, String), String> {
    let expected = || format!("expected {GRAMMAR}");
    let (name, rest) = word(text).ok_or_else(expected)?;
    let (field, rest) = if rest.starts_with('"') {
        (None, rest)
    } else {
        let (field, rest) = word(rest).ok_or_else(expected)?;
        (Some(field), rest)
    };
    let message = quoted(rest).ok_or_else(expected)?;
    if message.trim().is_empty() {
        return Err(format!("the message of `{name}` is empty: {}", expected()));
    }
    Ok((name, field, message))
}

/// The first word of `text` and what follows it, trimmed.
fn word(text: &str) -> Option<(&str, &str)> {
    let text = text.trim_start();
    if text.is_empty() || text.starts_with('"') {
        return None;
    }
    let end = text.find(char::is_whitespace).unwrap_or(text.len());
    Some((&text[..end], text[end..].trim_start()))
}

/// The double-quoted string `text` is, `\"` and `\\` escaped; `None` when
/// it is not one, or something follows it.
fn quoted(text: &str) -> Option<String> {
    let mut chars = text.strip_prefix('"')?.chars();
    let mut message = String::new();
    loop {
        match chars.next()? {
            '\\' => message.push(chars.next()?),
            '"' => break,
            other => message.push(other),
        }
    }
    chars.as_str().trim().is_empty().then_some(message)
}

/// Checks an annotation against the schema's constraints.
fn check(
    known: &BTreeMap<String, Vec<Known<'_>>>,
    name: &str,
    field: Option<&str>,
    message: String,
) -> Result<ConstraintMessage, String> {
    let candidates = known.get(name).map(Vec::as_slice).unwrap_or_default();
    let Some(constraint) = candidates.first() else {
        return Err(format!(
            "`{name}` is no constraint of the schema: `-- rok:constraint` names a CHECK, \
             unique or foreign key constraint, a unique index, or a name a function raises \
             (`RAISE … USING CONSTRAINT = '{name}'`)"
        ));
    };
    if candidates.len() > 1 {
        return Err(format!(
            "`{name}` names constraints of more than one table; rename one so a violation says which"
        ));
    }
    if let Some(field) = field {
        match &constraint.table {
            Some((table, columns)) if !columns.contains(&field) => {
                return Err(format!(
                    "`{field}` is no column of `{table}`, the table of `{name}`"
                ));
            }
            None if !known.values().flatten().any(|other| {
                other
                    .table
                    .as_ref()
                    .is_some_and(|(_, columns)| columns.contains(&field))
            }) =>
            {
                return Err(format!("`{field}` is no column of any table, for `{name}`"));
            }
            _ => {}
        }
    }
    Ok(ConstraintMessage {
        name: name.to_owned(),
        table: constraint
            .table
            .as_ref()
            .map(|(table, _)| (*table).to_owned()),
        field: field.map(str::to_owned),
        message,
        kind: constraint.kind,
    })
}

/// Every constraint name of the schema, with what it is.
fn known_constraints<'a>(
    sources: &[SchemaSource<'_>],
    schema: &'a Schema,
) -> BTreeMap<String, Vec<Known<'a>>> {
    let mut known: BTreeMap<String, Vec<Known<'a>>> = BTreeMap::new();
    for table in &schema.tables {
        let columns = || -> Vec<&'a str> {
            table
                .columns
                .iter()
                .map(|column| column.sql_name.as_str())
                .collect()
        };
        let mut add = |name: &str, kind: ConstraintKind| {
            known.entry(name.to_owned()).or_default().push(Known {
                kind,
                table: Some((table.sql_name.as_str(), columns())),
            });
        };
        for constraint in &table.key_constraints {
            add(&constraint.name, ConstraintKind::Unique);
        }
        for foreign_key in &table.foreign_keys {
            add(&foreign_key.name, ConstraintKind::ForeignKey);
        }
        for constraint in &table.named_constraints {
            let kind = if constraint.check.is_some() {
                ConstraintKind::Check
            } else {
                ConstraintKind::ForeignKey
            };
            add(&constraint.name, kind);
        }
    }
    for source in sources {
        for (index, table) in unique_indexes(source.sql) {
            let Some(table) = schema.tables.iter().find(|known| known.sql_name == table) else {
                continue;
            };
            let entry = known.entry(index).or_default();
            if entry.is_empty() {
                entry.push(Known {
                    kind: ConstraintKind::Unique,
                    table: Some((
                        table.sql_name.as_str(),
                        table
                            .columns
                            .iter()
                            .map(|column| column.sql_name.as_str())
                            .collect(),
                    )),
                });
            }
        }
        for name in raised_names(source.sql) {
            let entry = known.entry(name).or_default();
            if entry.is_empty() {
                entry.push(Known {
                    kind: ConstraintKind::Raised,
                    table: None,
                });
            }
        }
    }
    known
}

/// The unique indexes `sql` creates, as (index, table) names without
/// their schema: `CREATE UNIQUE INDEX [CONCURRENTLY] [IF NOT EXISTS]
/// <name> ON [ONLY] <table>`. An unnamed index has no name to annotate.
fn unique_indexes(sql: &str) -> Vec<(String, String)> {
    let words: Vec<String> = sql
        .split(|c: char| c.is_whitespace() || c == '(' || c == ';')
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect();
    let lower: Vec<String> = words.iter().map(|word| word.to_lowercase()).collect();
    let mut found = Vec::new();
    for start in 0..lower.len() {
        if lower[start..].len() < 5 || lower[start..start + 3] != ["create", "unique", "index"] {
            continue;
        }
        let mut at = start + 3;
        let skip = |at: &mut usize, run: &[&str]| {
            if lower
                .get(*at..*at + run.len())
                .is_some_and(|words| words == run)
            {
                *at += run.len();
            }
        };
        skip(&mut at, &["concurrently"]);
        skip(&mut at, &["if", "not", "exists"]);
        let Some(index) = words.get(at) else { continue };
        if lower.get(at + 1).map(String::as_str) != Some("on") {
            continue;
        }
        at += 2;
        skip(&mut at, &["only"]);
        let Some(table) = words.get(at) else { continue };
        found.push((leaf(index), leaf(table)));
    }
    found
}

/// A name's last part, unquoted: `"app"."offers"` is `offers`.
fn leaf(name: &str) -> String {
    let last = name.rsplit('.').next().unwrap_or(name);
    if let Some(quoted) = last
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        quoted.to_owned()
    } else {
        last.to_lowercase()
    }
}

/// The names `sql` raises as a constraint: each `CONSTRAINT = '<name>'`,
/// which only a `RAISE … USING` clause writes.
fn raised_names(sql: &str) -> Vec<String> {
    let lower = sql.to_ascii_lowercase();
    let mut names = Vec::new();
    let mut from = 0;
    while let Some(found) = lower[from..].find("constraint") {
        let after = from + found + "constraint".len();
        from = after;
        let rest = lower[after..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('\'') else {
            continue;
        };
        let Some(end) = rest.find('\'') else { continue };
        // The name as written: the offset of `rest` in `lower` is the
        // same in `sql`: ASCII lowercasing keeps every byte in place.
        let start = lower.len() - rest.len();
        names.push(sql[start..start + end].to_owned());
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_is_quoted_and_may_escape_its_quotes() {
        assert_eq!(
            parse(r#"orders_total_check total "Say \"no\"." "#),
            Ok((
                "orders_total_check",
                Some("total"),
                "Say \"no\".".to_owned()
            ))
        );
        assert_eq!(
            parse(r#"orders_fkey "Gone.""#),
            Ok(("orders_fkey", None, "Gone.".to_owned()))
        );
        assert!(parse("orders_fkey").is_err());
        assert!(parse(r#"orders_fkey "open"#).is_err());
        assert!(parse(r#"orders_fkey "Gone." trailing"#).is_err());
    }

    #[test]
    fn unique_indexes_and_raised_names_are_read_from_the_text() {
        assert_eq!(
            unique_indexes(
                "CREATE UNIQUE INDEX IF NOT EXISTS one_live ON ONLY app.offers (drop_id);\n\
                 create unique index on offers (id);"
            ),
            [("one_live".to_owned(), "offers".to_owned())]
        );
        assert_eq!(
            raised_names(
                "RAISE EXCEPTION 'x' USING ERRCODE = 'check_violation', CONSTRAINT = 'Window_Open';\n\
                 CONSTRAINT offers_pkey PRIMARY KEY (id)"
            ),
            ["Window_Open"]
        );
    }
}
