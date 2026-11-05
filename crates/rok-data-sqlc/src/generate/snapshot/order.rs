//! The order the snapshot lists the schema's statements in: one that loads
//! into an empty database.
//!
//! The inputs are read in file-name order, and a statement may name an
//! object a later file creates (`users.home_city_id` in `0001` referencing
//! `cities` in `0007`). The migration diff (rok-data-codegen) avoids that
//! by rewriting the schema (every table first, then its foreign keys); the
//! snapshot keeps each statement as written instead, so it moves whole
//! statements: a statement waits until every object it names has been
//! created, and otherwise keeps its place. An already loadable order comes
//! back unchanged.
//!
//! What a statement creates is read from its head (`CREATE [OR REPLACE]
//! TABLE name`, `ALTER TABLE name`, …); what it needs is every name in its
//! text, string and function bodies included, that another statement
//! creates. Naming too much only makes a statement wait for one it does not
//! need; a cycle (two tables referencing each other) keeps the input order
//! for the statements in it.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap};

use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::tokenizer::{Location, Token, TokenWithSpan, Tokenizer, Word};

use crate::parser::postgres::split_statements;

/// `sources` joined into one script, each starting on a new line, with the
/// statements in an order that loads. Each statement's text, with the
/// comments before it, is kept verbatim. A source the tokenizer cannot read
/// leaves every source in its given order (the parse reports it).
pub(super) fn ordered(sources: &[&str]) -> String {
    let Some(statements) = sources
        .iter()
        .map(|sql| statements(sql))
        .collect::<Option<Vec<_>>>()
    else {
        return joined(sources.iter().copied());
    };
    let statements: Vec<Statement<'_>> = statements.into_iter().flatten().collect();
    let order = load_order(&statements);
    if order.iter().copied().eq(0..statements.len()) {
        return joined(sources.iter().copied());
    }
    joined(order.into_iter().map(|index| statements[index].text))
}

/// `texts` concatenated, each starting on a new line.
fn joined<'a>(texts: impl Iterator<Item = &'a str>) -> String {
    let mut sql = String::new();
    for text in texts {
        if !sql.is_empty() && !sql.ends_with('\n') {
            sql.push('\n');
        }
        sql.push_str(text);
    }
    sql
}

/// One statement of an input: its text and the names it creates and uses.
#[derive(Debug)]
struct Statement<'a> {
    /// From the end of the previous statement through this one's `;` and
    /// the rest of its line.
    text: &'a str,
    creates: Vec<String>,
    uses: BTreeSet<String>,
}

/// The statements of `sql`, covering all of it: concatenated, their texts
/// are `sql`. `None` when the tokenizer cannot read it.
fn statements(sql: &str) -> Option<Vec<Statement<'_>>> {
    let tokens = Tokenizer::new(&PostgreSqlDialect {}, sql)
        .tokenize_with_location()
        .ok()?;
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(sql.match_indices('\n').map(|(at, _)| at + 1))
        .collect();
    let mut statements = Vec::new();
    let (mut first_token, mut start) = (0, 0);
    for chunk in split_statements(&tokens) {
        // The chunks are the tokens between top-level semicolons, so the
        // one after a chunk is its semicolon (none after the last).
        let semicolon = tokens.get(first_token + chunk.len());
        first_token += chunk.len() + 1;
        let end = match semicolon {
            Some(semicolon) => line_end(sql, offset(sql, &line_starts, semicolon.span.start)? + 1),
            None => sql.len(),
        };
        let (creates, uses) = names(chunk);
        statements.push(Statement {
            text: &sql[start..end],
            creates,
            uses,
        });
        start = end;
    }
    Some(statements)
}

/// The byte offset of `location` (1-based line, 1-based column in chars).
fn offset(sql: &str, line_starts: &[usize], location: Location) -> Option<usize> {
    let line_start = *line_starts.get(usize::try_from(location.line).ok()?.checked_sub(1)?)?;
    let column = usize::try_from(location.column).ok()?.checked_sub(1)?;
    let line = &sql[line_start..];
    let within = line
        .char_indices()
        .nth(column)
        .map_or(line.len(), |(at, _)| at);
    Some(line_start + within)
}

/// `at` moved past the blanks and a `--` comment that end its line, and the
/// newline: a statement keeps the rest of the line its `;` is on.
fn line_end(sql: &str, at: usize) -> usize {
    let rest = &sql[at..];
    let blanks = rest.len() - rest.trim_start_matches([' ', '\t', '\r']).len();
    let rest = &rest[blanks..];
    if rest.starts_with('\n') {
        return at + blanks + 1;
    }
    if rest.starts_with("--") {
        return rest
            .find('\n')
            .map_or(sql.len(), |newline| at + blanks + newline + 1);
    }
    at
}

/// The kinds of object a `CREATE` or `ALTER` head names.
const KINDS: &[&str] = &[
    "AGGREGATE",
    "DOMAIN",
    "EXTENSION",
    "FUNCTION",
    "PROCEDURE",
    "SCHEMA",
    "SEQUENCE",
    "TABLE",
    "TYPE",
    "VIEW",
];

/// The words between `CREATE` and the kind.
const MODIFIERS: &[&str] = &[
    "GLOBAL",
    "LOCAL",
    "MATERIALIZED",
    "OR",
    "RECURSIVE",
    "REPLACE",
    "TEMP",
    "TEMPORARY",
    "UNLOGGED",
];

/// What a statement creates (or changes: an `ALTER` counts, so whatever
/// needs the change waits for it) and every name it uses.
fn names(tokens: &[TokenWithSpan]) -> (Vec<String>, BTreeSet<String>) {
    let significant: Vec<&Token> = tokens
        .iter()
        .map(|token| &token.token)
        .filter(|token| !matches!(token, Token::Whitespace(_) | Token::EOF))
        .collect();
    let mut creates = Vec::new();
    let mut rest = significant.as_slice();
    if let Some((head, after)) = rest.split_first()
        && (keyword(head, "CREATE") || keyword(head, "ALTER"))
    {
        rest = after;
        while rest
            .first()
            .is_some_and(|t| MODIFIERS.iter().any(|m| keyword(t, m)))
        {
            rest = &rest[1..];
        }
        if rest
            .first()
            .is_some_and(|t| KINDS.iter().any(|k| keyword(t, k)))
        {
            rest = &rest[1..];
            while rest.first().is_some_and(|t| {
                ["IF", "NOT", "EXISTS", "ONLY"]
                    .iter()
                    .any(|k| keyword(t, k))
            }) {
                rest = &rest[1..];
            }
            creates.extend(leaf_name(rest));
            if keyword(head, "ALTER")
                && let Some(at) = rest
                    .windows(2)
                    .position(|w| keyword(w[0], "RENAME") && keyword(w[1], "TO"))
            {
                creates.extend(leaf_name(&rest[at + 2..]));
            }
        }
    }
    let mut uses = BTreeSet::new();
    for token in &significant {
        match token {
            Token::Word(word) => {
                uses.insert(name(word));
            }
            Token::SingleQuotedString(text)
            | Token::EscapedStringLiteral(text)
            | Token::NationalStringLiteral(text)
            | Token::UnicodeStringLiteral(text) => uses.extend(words(text)),
            Token::DollarQuotedString(text) => uses.extend(words(&text.value)),
            _ => {}
        }
    }
    // A `CREATE` needs no other statement for its own name; an `ALTER`
    // needs the one that created (or last changed) it.
    if significant
        .first()
        .is_some_and(|head| keyword(head, "CREATE"))
    {
        for created in &creates {
            uses.remove(created);
        }
    }
    (creates, uses)
}

/// The last part of the dotted name `tokens` start with.
fn leaf_name(tokens: &[&Token]) -> Option<String> {
    let mut leaf = None;
    let mut rest = tokens;
    while let Some((Token::Word(word), after)) = rest.split_first() {
        leaf = Some(name(word));
        match after.split_first() {
            Some((Token::Period, after)) => rest = after,
            _ => break,
        }
    }
    leaf
}

/// A name as PostgreSQL folds it: quoted names keep their case.
fn name(word: &Word) -> String {
    match word.quote_style {
        Some(_) => word.value.clone(),
        None => word.value.to_lowercase(),
    }
}

/// The identifier-like words of a string or function body, folded.
fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .filter(|word| {
            word.chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
        })
        .map(str::to_lowercase)
}

fn keyword(token: &Token, word: &str) -> bool {
    matches!(token, Token::Word(w) if w.quote_style.is_none() && w.value.eq_ignore_ascii_case(word))
}

/// The statements' indexes in an order that loads: each one after the
/// statements it needs, and otherwise as early as its input position
/// allows. A statement needs, for each name it uses, the nearest earlier
/// statement creating it, or the first later one when none is earlier.
fn load_order(statements: &[Statement<'_>]) -> Vec<usize> {
    let mut creators: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, statement) in statements.iter().enumerate() {
        for created in &statement.creates {
            creators.entry(created).or_default().push(index);
        }
    }
    let mut waiting_for = vec![0usize; statements.len()];
    let mut needed_by: Vec<Vec<usize>> = vec![Vec::new(); statements.len()];
    for (index, statement) in statements.iter().enumerate() {
        let needs: BTreeSet<usize> = statement
            .uses
            .iter()
            .filter_map(|used| {
                let creators = creators.get(used.as_str())?;
                let earlier = creators.iter().rev().find(|&&c| c < index);
                earlier
                    .or_else(|| creators.iter().find(|&&c| c > index))
                    .copied()
            })
            .collect();
        waiting_for[index] = needs.len();
        for need in needs {
            needed_by[need].push(index);
        }
    }

    let mut order = Vec::with_capacity(statements.len());
    let mut placed = vec![false; statements.len()];
    let mut ready: BinaryHeap<Reverse<usize>> = (0..statements.len())
        .filter(|&index| waiting_for[index] == 0)
        .map(Reverse)
        .collect();
    while order.len() < statements.len() {
        // Stuck in a cycle: the earliest statement left goes next.
        let next = match ready.pop() {
            Some(Reverse(index)) => index,
            None => (0..statements.len())
                .find(|&index| !placed[index])
                .expect("a statement is left"),
        };
        if placed[next] {
            continue;
        }
        placed[next] = true;
        order.push(next);
        for &dependent in &needed_by[next] {
            waiting_for[dependent] -= 1;
            if waiting_for[dependent] == 0 && !placed[dependent] {
                ready.push(Reverse(dependent));
            }
        }
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    const USERS: &str = "CREATE TABLE users (\n    id BIGINT PRIMARY KEY,\n    home_city_id BIGINT REFERENCES cities (id)\n);\n\
         CREATE INDEX users_home_city_id_idx ON users (home_city_id);\n";
    const CITIES: &str =
        "-- Cities a user can live in.\nCREATE TABLE cities (id BIGINT PRIMARY KEY);\n";

    #[test]
    fn a_table_waits_for_the_table_a_later_file_creates() {
        assert_eq!(
            ordered(&[USERS, CITIES]),
            "-- Cities a user can live in.\nCREATE TABLE cities (id BIGINT PRIMARY KEY);\n\
             CREATE TABLE users (\n    id BIGINT PRIMARY KEY,\n    home_city_id BIGINT REFERENCES cities (id)\n);\n\
             CREATE INDEX users_home_city_id_idx ON users (home_city_id);\n"
        );
    }

    #[test]
    fn a_loadable_order_comes_back_unchanged() {
        let functions = "CREATE FUNCTION touch() RETURNS trigger LANGUAGE plpgsql AS $$\nBEGIN\n  NEW.updated_at := now();\n  RETURN NEW;\nEND;\n$$;";
        let trigger = "CREATE TRIGGER cities_touch BEFORE UPDATE ON cities\n  FOR EACH ROW EXECUTE FUNCTION touch();  -- keep it fresh\n\n\n";
        assert_eq!(
            ordered(&[functions, CITIES, trigger, USERS]),
            format!("{functions}\n{CITIES}{trigger}{USERS}")
        );
    }

    #[test]
    fn statements_on_a_waiting_table_wait_with_it() {
        let functions = "CREATE FUNCTION touch() RETURNS trigger LANGUAGE plpgsql AS $$\nBEGIN\n  RETURN NEW;\nEND;\n$$;\n";
        let users = format!(
            "{USERS}ALTER TABLE users ADD COLUMN updated_at TIMESTAMPTZ;\n\
             CREATE TRIGGER users_touch BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION touch();\n\
             CREATE VIEW user_cities AS SELECT updated_at FROM users;\n"
        );
        let snapshot = ordered(&[functions, &users, CITIES]);
        let at = |needle: &str| {
            snapshot
                .find(needle)
                .unwrap_or_else(|| panic!("{needle} missing:\n{snapshot}"))
        };
        assert!(
            at("CREATE FUNCTION touch") < at("CREATE TABLE cities"),
            "{snapshot}"
        );
        assert!(
            at("CREATE TABLE cities") < at("CREATE TABLE users"),
            "{snapshot}"
        );
        assert!(at("CREATE TABLE users") < at("CREATE INDEX"), "{snapshot}");
        assert!(at("CREATE INDEX") < at("ALTER TABLE users"), "{snapshot}");
        assert!(at("ALTER TABLE users") < at("CREATE TRIGGER"), "{snapshot}");
        assert!(at("ALTER TABLE users") < at("CREATE VIEW"), "{snapshot}");
    }

    #[test]
    fn a_cycle_keeps_every_statement_in_input_order() {
        let a = "CREATE TABLE a (id BIGINT PRIMARY KEY, b_id BIGINT REFERENCES b (id));\n";
        let b = "CREATE TABLE b (id BIGINT PRIMARY KEY, a_id BIGINT REFERENCES a (id));\n";
        assert_eq!(ordered(&[a, b]), format!("{a}{b}"));
    }

    #[test]
    fn schema_qualified_and_quoted_names_are_matched() {
        let users = "CREATE TABLE auth.\"Users\" (id BIGINT PRIMARY KEY, city_id BIGINT REFERENCES geo.cities (id));\n";
        let cities = "CREATE TABLE IF NOT EXISTS geo.cities (id BIGINT PRIMARY KEY);\n";
        let schemas = "CREATE SCHEMA auth;\nCREATE SCHEMA geo;\n";
        assert_eq!(
            ordered(&[users, cities, schemas]),
            format!("CREATE SCHEMA auth;\nCREATE SCHEMA geo;\n{cities}{users}")
        );
    }
}
