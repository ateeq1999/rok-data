//! `:page` queries: one `SELECT` with its `WHERE` and `ORDER BY`, run a page
//! at a time with the total across every page.
//!
//! The query is checked here (a `SELECT`, ordered, without its own
//! `LIMIT`/`OFFSET`/`FETCH`), and its total is counted by
//! [`count_sql`]: `SELECT count(*) FROM (<the query without its ORDER BY>) q`.
//! The rows are the query itself, which `rok_data::Raw::fetch_page` runs
//! with `LIMIT`/`OFFSET` appended.

use sqlparser::ast::{LimitClause, SetExpr, Statement};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::tokenizer::{Location, Token, Tokenizer};

/// Fails unless `statement` can be paged: a query with an `ORDER BY` and
/// no `LIMIT`, `OFFSET` or `FETCH` of its own.
pub(crate) fn check(statement: &Statement) -> Result<(), String> {
    let not_a_select =
        |kind: &str| format!("a `:page` query is a `SELECT`; this one is a `{kind}`");
    let query = match statement {
        Statement::Query(query) => query,
        Statement::Insert(_) => return Err(not_a_select("INSERT")),
        Statement::Update { .. } => return Err(not_a_select("UPDATE")),
        Statement::Delete(_) => return Err(not_a_select("DELETE")),
        _ => return Err(not_a_select("statement other than a query")),
    };
    match query.body.as_ref() {
        SetExpr::Insert(_) => return Err(not_a_select("INSERT")),
        SetExpr::Update(_) => return Err(not_a_select("UPDATE")),
        SetExpr::Delete(_) => return Err(not_a_select("DELETE")),
        _ => {}
    }
    let own = match &query.limit_clause {
        Some(LimitClause::LimitOffset { limit: Some(_), .. })
        | Some(LimitClause::OffsetCommaLimit { .. }) => Some("LIMIT"),
        Some(LimitClause::LimitOffset {
            offset: Some(_), ..
        }) => Some("OFFSET"),
        _ if query.fetch.is_some() => Some("FETCH"),
        _ => None,
    };
    if let Some(clause) = own {
        return Err(format!(
            "a `:page` query takes no `{clause}`: the generated function adds `LIMIT`/`OFFSET` from its `page` and `per_page`"
        ));
    }
    if query.order_by.is_none() {
        return Err("a `:page` query needs an `ORDER BY`: without one its pages hold rows in no particular order, so a row can show on two pages or none".to_owned());
    }
    Ok(())
}

/// The statement counting a `:page` query's rows across every page: the
/// query without its top-level `ORDER BY` (sorting does not change a
/// count), as a subquery. The count binds the query's own parameters.
pub fn count_sql(sql: &str) -> String {
    let rows = sql[..order_by_start(sql).unwrap_or(sql.len())].trim_end();
    let body = rows
        .lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("    {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("SELECT count(*) FROM (\n{body}\n) q")
}

/// The byte offset of the `ORDER` of the query's own `ORDER BY`: the last
/// one outside parentheses (an aggregate's or a window's is inside them).
fn order_by_start(sql: &str) -> Option<usize> {
    let tokens = Tokenizer::new(&PostgreSqlDialect {}, sql)
        .tokenize_with_location()
        .ok()?;
    let significant: Vec<_> = tokens
        .iter()
        .filter(|token| !matches!(token.token, Token::Whitespace(_)))
        .collect();
    let mut depth = 0usize;
    let mut found = None;
    for (at, token) in significant.iter().enumerate() {
        match &token.token {
            Token::LParen => depth += 1,
            Token::RParen => depth = depth.saturating_sub(1),
            Token::Word(word)
                if depth == 0
                    && word.quote_style.is_none()
                    && word.value.eq_ignore_ascii_case("order")
                    && significant.get(at + 1).is_some_and(|next| {
                        matches!(&next.token, Token::Word(by) if by.quote_style.is_none() && by.value.eq_ignore_ascii_case("by"))
                    }) =>
            {
                found = Some(token.span.start);
            }
            _ => {}
        }
    }
    byte_offset(sql, found?)
}

/// The byte offset of `location` (1-based line, 1-based column in chars).
fn byte_offset(sql: &str, location: Location) -> Option<usize> {
    let line = usize::try_from(location.line).ok()?.checked_sub(1)?;
    let column = usize::try_from(location.column).ok()?.checked_sub(1)?;
    let line_start: usize = sql.split_inclusive('\n').take(line).map(str::len).sum();
    let rest = &sql[line_start..];
    Some(
        line_start
            + rest
                .char_indices()
                .nth(column)
                .map_or(rest.len(), |(at, _)| at),
    )
}

#[cfg(test)]
mod tests {
    use super::count_sql;

    #[test]
    fn the_count_drops_the_query_s_own_order_by() {
        assert_eq!(
            count_sql(
                "SELECT id\nFROM bookings\nWHERE starts_on >= $1\nORDER BY starts_on DESC, id"
            ),
            "SELECT count(*) FROM (\n    SELECT id\n    FROM bookings\n    WHERE starts_on >= $1\n) q"
        );
    }

    /// An aggregate's or a window's `ORDER BY` is part of a column, and a
    /// CTE's is part of the CTE.
    #[test]
    fn the_count_keeps_order_bys_inside_parentheses() {
        let sql = "WITH recent AS (SELECT id FROM t ORDER BY id LIMIT 5)\nSELECT string_agg(name, ',' ORDER BY name) AS names, row_number() OVER (ORDER BY id) AS n\nFROM recent GROUP BY id order  by n";
        assert_eq!(
            count_sql(sql),
            "SELECT count(*) FROM (\n    WITH recent AS (SELECT id FROM t ORDER BY id LIMIT 5)\n    SELECT string_agg(name, ',' ORDER BY name) AS names, row_number() OVER (ORDER BY id) AS n\n    FROM recent GROUP BY id\n) q"
        );
    }
}
