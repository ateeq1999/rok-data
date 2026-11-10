//! `-- rok:constraint <name> [<field>] "<message>"`: what a broken
//! constraint means, written anywhere in the schema dir, and the
//! `constraints.rs` table the data layer gets from it.
//!
//! A name is a table's check, unique or foreign-key constraint, a unique
//! index, or a name a trigger raises (`RAISE … USING CONSTRAINT =
//! '<name>'`). The field, when given, is a column of the constraint's table
//! (of any table, for a raised name): the error the App answers names it.
//! An unknown name or field, a malformed line and a second annotation of
//! one name fail the run, naming the file and line.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::generator::HEADER;
use crate::report::display_path;
use crate::schema::Schema;

/// What breaking a constraint is: a check (422 on its field) or a
/// conflict with other rows (409).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A `CHECK`, or a name a trigger raises.
    Check,
    /// A primary key, unique constraint or unique index.
    Unique,
    /// A foreign key.
    ForeignKey,
}

impl Kind {
    fn variant(self) -> &'static str {
        match self {
            Kind::Check => "Check",
            Kind::Unique => "Unique",
            Kind::ForeignKey => "ForeignKey",
        }
    }
}

/// One annotation, checked against the schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConstraintMessage {
    pub name: String,
    pub kind: Kind,
    pub field: Option<String>,
    pub message: String,
}

/// A name the schema declares: its kind and the columns of its table
/// (`None` for a raised name, whose table is not known).
struct Known {
    kind: Kind,
    columns: Option<Vec<String>>,
}

/// Every annotation of `sources`, checked against `schema`, by name.
pub(crate) fn read(
    sources: &[(PathBuf, String)],
    schema: &Schema,
) -> Result<Vec<ConstraintMessage>> {
    let known = known_names(sources, schema);
    let every_column: Vec<String> = schema
        .tables
        .iter()
        .flat_map(|table| table.columns.iter().map(|column| column.sql_name.clone()))
        .collect();
    let mut found: BTreeMap<String, ConstraintMessage> = BTreeMap::new();
    for (path, sql) in sources {
        for (index, line) in sql.lines().enumerate() {
            let Some(rest) = line.trim().strip_prefix("-- rok:constraint") else {
                continue;
            };
            let at = format!("{}:{}", display_path(path), index + 1);
            let (name, field, message) = annotation(rest)
                .map_err(|reason| anyhow::anyhow!("{at}: `-- rok:constraint`: {reason}"))?;
            let Some(declared) = known.get(&name) else {
                bail!(
                    "{at}: `-- rok:constraint {name}`: the schema has no constraint, unique \
                     index or raised constraint named `{name}`"
                );
            };
            if let Some(field) = &field {
                let columns = declared.columns.as_ref().unwrap_or(&every_column);
                if !columns.contains(field) {
                    bail!(
                        "{at}: `-- rok:constraint {name} {field}`: `{field}` is not a column of \
                         the constraint's table"
                    );
                }
            }
            if found.contains_key(&name) {
                bail!("{at}: `-- rok:constraint {name}`: the constraint has a message already");
            }
            found.insert(
                name.clone(),
                ConstraintMessage {
                    name,
                    kind: declared.kind,
                    field,
                    message,
                },
            );
        }
    }
    Ok(found.into_values().collect())
}

/// `<name> [<field>] "<message>"`.
fn annotation(text: &str) -> std::result::Result<(String, Option<String>, String), String> {
    let usage = "write `-- rok:constraint <name> [<field>] \"<message>\"`";
    let text = text.trim();
    let Some(quote) = text.find('"') else {
        return Err(format!("no message; {usage}"));
    };
    let words: Vec<&str> = text[..quote].split_whitespace().collect();
    let (name, field) = match words.as_slice() {
        [name] => (*name, None),
        [name, field] => (*name, Some((*field).to_owned())),
        _ => return Err(usage.to_owned()),
    };
    let message =
        quoted(&text[quote..]).ok_or_else(|| format!("the message is not quoted; {usage}"))?;
    if message.trim().is_empty() {
        return Err("the message is empty".to_owned());
    }
    Ok((name.to_owned(), field, message))
}

/// The text of `"…"` (with `\"` and `\\` escapes), when nothing follows it.
fn quoted(text: &str) -> Option<String> {
    let mut chars = text.strip_prefix('"')?.chars();
    let mut out = String::new();
    loop {
        match chars.next()? {
            '\\' => out.push(chars.next()?),
            '"' => break,
            c => out.push(c),
        }
    }
    chars.as_str().trim().is_empty().then_some(out)
}

/// The names the schema declares, with their kind and table's columns.
fn known_names(sources: &[(PathBuf, String)], schema: &Schema) -> BTreeMap<String, Known> {
    let mut known = BTreeMap::new();
    for table in &schema.tables {
        let columns: Vec<String> = table.columns.iter().map(|c| c.sql_name.clone()).collect();
        let mut add = |name: &str, kind: Kind| {
            known.insert(
                name.to_owned(),
                Known {
                    kind,
                    columns: Some(columns.clone()),
                },
            );
        };
        for key in &table.key_constraints {
            add(&key.name, Kind::Unique);
        }
        for foreign_key in &table.foreign_keys {
            add(&foreign_key.name, Kind::ForeignKey);
        }
        for named in &table.named_constraints {
            let kind = if named.check.is_some() {
                Kind::Check
            } else {
                Kind::ForeignKey
            };
            add(&named.name, kind);
        }
    }
    for (_, sql) in sources {
        for (name, table) in unique_indexes(sql) {
            let columns = schema
                .tables
                .iter()
                .find(|t| t.sql_name == table)
                .map(|t| t.columns.iter().map(|c| c.sql_name.clone()).collect());
            known.entry(name).or_insert(Known {
                kind: Kind::Unique,
                columns,
            });
        }
        for name in raised(sql) {
            known.entry(name).or_insert(Known {
                kind: Kind::Check,
                columns: None,
            });
        }
    }
    known
}

/// The words of `sql`, lowercased, with their original text: enough to
/// find `CREATE UNIQUE INDEX <name> ON <table>`.
fn words(sql: &str) -> Vec<&str> {
    sql.split(|c: char| c.is_whitespace() || c == '(' || c == ';')
        .filter(|word| !word.is_empty())
        .collect()
}

/// `(index, table)` of each `CREATE UNIQUE INDEX` in `sql`.
fn unique_indexes(sql: &str) -> Vec<(String, String)> {
    let words = words(sql);
    let mut out = Vec::new();
    for (i, window) in words.windows(3).enumerate() {
        if !(window[0].eq_ignore_ascii_case("create")
            && window[1].eq_ignore_ascii_case("unique")
            && window[2].eq_ignore_ascii_case("index"))
        {
            continue;
        }
        let mut rest = words[i + 3..].iter().copied().peekable();
        if rest
            .peek()
            .is_some_and(|w| w.eq_ignore_ascii_case("concurrently"))
        {
            rest.next();
        }
        if rest.peek().is_some_and(|w| w.eq_ignore_ascii_case("if")) {
            rest.next();
            rest.next();
            rest.next();
        }
        let Some(name) = rest.next() else { continue };
        if !rest.next().is_some_and(|w| w.eq_ignore_ascii_case("on")) {
            continue;
        }
        let mut table = rest.next().unwrap_or_default();
        if table.eq_ignore_ascii_case("only") {
            table = rest.next().unwrap_or_default();
        }
        let table = table.rsplit('.').next().unwrap_or(table);
        out.push((unquote(name), unquote(table)));
    }
    out
}

/// The names a trigger raises: `CONSTRAINT = '<name>'` in a function body.
fn raised(sql: &str) -> Vec<String> {
    let mut out = Vec::new();
    let upper = sql.to_ascii_uppercase();
    let mut from = 0;
    while let Some(found) = upper[from..].find("CONSTRAINT") {
        let start = from + found + "CONSTRAINT".len();
        from = start;
        let rest = sql[start..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('\'') else {
            continue;
        };
        if let Some(end) = rest.find('\'') {
            out.push(rest[..end].to_owned());
        }
    }
    out
}

fn unquote(name: &str) -> String {
    name.trim_matches('"').to_owned()
}

/// `constraints.rs`: the table of what each annotated constraint means.
pub(crate) fn render(messages: &[ConstraintMessage]) -> String {
    let mut out = format!(
        "{HEADER}\n//! What each annotated constraint means (`-- rok:constraint` in the\n\
         //! schema): the App's error mapping answers a violation of one with its\n\
         //! message, on its field when it names one.\n\n\
         /// What breaking a constraint is.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub enum ConstraintKind {{\n    \
         /// A `CHECK`, or a name a trigger raises: the value is refused.\n    \
         Check,\n    \
         /// A primary key, unique constraint or unique index: the value is taken.\n    \
         Unique,\n    \
         /// A foreign key: the change breaks a reference.\n    \
         ForeignKey,\n}}\n\n\
         /// One annotated constraint.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub struct Constraint {{\n    \
         /// Its name, as a violation reports it.\n    \
         pub name: &'static str,\n    \
         /// What breaking it is.\n    \
         pub kind: ConstraintKind,\n    \
         /// The field the error names, if any.\n    \
         pub field: Option<&'static str>,\n    \
         /// What the error says.\n    \
         pub message: &'static str,\n}}\n\n\
         /// Every annotated constraint, by name.\n\
         pub const CONSTRAINTS: &[Constraint] = &["
    );
    if messages.is_empty() {
        out.push_str("];\n");
    } else {
        out.push('\n');
        for message in messages {
            let field = match &message.field {
                Some(field) => format!("Some({field:?})"),
                None => "None".to_owned(),
            };
            out.push_str(&format!(
                "    Constraint {{\n        name: {:?},\n        kind: ConstraintKind::{},\n        \
                 field: {field},\n        message: {:?},\n    }},\n",
                message.name,
                message.kind.variant(),
                message.message,
            ));
        }
        out.push_str("];\n");
    }
    out.push_str(
        "\n/// The annotated constraint named `name`, as a violation reports it.\n\
         #[must_use]\n\
         pub fn constraint(name: &str) -> Option<&'static Constraint> {\n    \
         CONSTRAINTS\n        .iter()\n        .find(|constraint| constraint.name == name)\n}\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn read_sql(sql: &str) -> Result<Vec<ConstraintMessage>> {
        let parsed = parser::parse_schema(sql)?;
        read(
            &[(PathBuf::from("schema.sql"), sql.to_owned())],
            &parsed.schema,
        )
    }

    const SCHEMA: &str = r#"
CREATE TABLE orgs (id BIGSERIAL PRIMARY KEY, slug TEXT NOT NULL UNIQUE);
CREATE TABLE drops (
    id BIGSERIAL PRIMARY KEY,
    org_id BIGINT NOT NULL REFERENCES orgs (id),
    month INT NOT NULL CHECK (month BETWEEN 1 AND 12),
    code TEXT NOT NULL
);
CREATE UNIQUE INDEX drops_code_key ON drops (lower(code));
CREATE FUNCTION no_past() RETURNS trigger AS $$
BEGIN
  RAISE EXCEPTION 'past' USING CONSTRAINT = 'drops_not_in_past';
END $$ LANGUAGE plpgsql;
-- rok:constraint drops_month_check month "Pick a month from 1 to 12."
-- rok:constraint orgs_slug_key slug "That address is taken."
-- rok:constraint drops_org_id_fkey "The organization doesn't exist."
-- rok:constraint drops_code_key code "That code is in use."
-- rok:constraint drops_not_in_past month "A drop can't be in the past."
"#;

    #[test]
    fn every_kind_of_constraint_takes_a_message() {
        let messages = read_sql(SCHEMA).unwrap_or_else(|err| panic!("{err}"));
        let found: Vec<(&str, Kind, Option<&str>)> = messages
            .iter()
            .map(|m| (m.name.as_str(), m.kind, m.field.as_deref()))
            .collect();
        assert_eq!(
            found,
            [
                ("drops_code_key", Kind::Unique, Some("code")),
                ("drops_month_check", Kind::Check, Some("month")),
                ("drops_not_in_past", Kind::Check, Some("month")),
                ("drops_org_id_fkey", Kind::ForeignKey, None),
                ("orgs_slug_key", Kind::Unique, Some("slug")),
            ]
        );
        assert_eq!(messages[1].message, "Pick a month from 1 to 12.");
    }

    #[test]
    fn mistakes_fail_naming_the_line() {
        let base = "CREATE TABLE orgs (id BIGSERIAL PRIMARY KEY, slug TEXT UNIQUE);\n";
        for (line, expected) in [
            (
                "-- rok:constraint orgs_nope \"x\"",
                "schema.sql:2: `-- rok:constraint orgs_nope`: the schema has no constraint",
            ),
            (
                "-- rok:constraint orgs_slug_key name \"x\"",
                "`name` is not a column of the constraint's table",
            ),
            ("-- rok:constraint orgs_slug_key", "no message"),
            ("-- rok:constraint orgs_slug_key \"x\" extra", "not quoted"),
            (
                "-- rok:constraint a b c \"x\"",
                "write `-- rok:constraint <name>",
            ),
        ] {
            let error = read_sql(&format!("{base}{line}\n"))
                .unwrap_err()
                .to_string();
            assert!(error.contains(expected), "{line}: {error}");
        }
        let twice = format!(
            "{base}-- rok:constraint orgs_slug_key \"a\"\n-- rok:constraint orgs_slug_key \"b\"\n"
        );
        let error = read_sql(&twice).unwrap_err().to_string();
        assert!(error.contains("has a message already"), "{error}");
    }

    #[test]
    fn the_table_lists_each_constraint() {
        let messages = read_sql(SCHEMA).unwrap_or_else(|err| panic!("{err}"));
        let text = render(&messages);
        assert!(text.contains(
            "    Constraint {\n        name: \"drops_month_check\",\n        \
             kind: ConstraintKind::Check,\n        field: Some(\"month\"),\n        \
             message: \"Pick a month from 1 to 12.\",\n    },\n"
        ));
        assert!(text.contains("pub fn constraint(name: &str) -> Option<&'static Constraint> {"));
        assert!(render(&[]).contains("pub const CONSTRAINTS: &[Constraint] = &[];\n"));
    }

    #[test]
    fn a_message_takes_escaped_quotes() {
        assert_eq!(
            annotation(r#" orgs_slug_key slug "the \"slug\" is taken""#),
            Ok((
                "orgs_slug_key".to_owned(),
                Some("slug".to_owned()),
                "the \"slug\" is taken".to_owned()
            ))
        );
    }
}
