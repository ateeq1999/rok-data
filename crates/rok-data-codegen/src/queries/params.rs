//! Guessing the parameter names behind the placeholders of a query.

use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Token, Tokenizer, Word};

use crate::naming;

pub(crate) fn words(sql: &str) -> Result<Vec<Token>, String> {
    let tokens = Tokenizer::new(&PostgreSqlDialect {}, sql)
        .tokenize()
        .map_err(|e| e.to_string())?;
    Ok(tokens
        .into_iter()
        .filter(|t| !matches!(t, Token::Whitespace(_)))
        .collect())
}

pub(crate) fn word_name(w: &Word) -> String {
    if w.quote_style.is_some() {
        w.value.clone()
    } else {
        w.value.to_ascii_lowercase()
    }
}

/// A possibly qualified name starting at `i`: (`schema.table`, next index).
pub(crate) fn name_at(tokens: &[Token], mut i: usize) -> Option<(String, usize)> {
    let mut parts = Vec::new();
    while let Some(Token::Word(w)) = tokens.get(i) {
        parts.push(word_name(w));
        i += 1;
        if tokens.get(i) == Some(&Token::Period) {
            i += 1;
        } else {
            break;
        }
    }
    (!parts.is_empty()).then(|| (parts.join("."), i))
}

pub(crate) fn placeholder(t: &Token) -> Option<usize> {
    match t {
        Token::Placeholder(p) => p.strip_prefix('$')?.parse().ok(),
        _ => None,
    }
}

pub(crate) fn is_keyword(t: Option<&Token>, k: Keyword) -> bool {
    matches!(t, Some(Token::Word(w)) if w.keyword == k)
}

pub(crate) fn is_comparison(t: Option<&Token>) -> bool {
    matches!(
        t,
        Some(Token::Eq | Token::Neq | Token::Lt | Token::Gt | Token::LtEq | Token::GtEq)
    ) || [Keyword::LIKE, Keyword::ILIKE]
        .iter()
        .any(|k| is_keyword(t, *k))
}

/// The column name ending at index `end` (`users.email` -> `email`).
pub(crate) fn column_before(tokens: &[Token], end: usize) -> Option<String> {
    const STRUCTURAL: &[Keyword] = &[
        Keyword::AND,
        Keyword::OR,
        Keyword::NOT,
        Keyword::WHERE,
        Keyword::ON,
        Keyword::SET,
        Keyword::SELECT,
        Keyword::FROM,
        Keyword::BY,
        Keyword::WHEN,
        Keyword::THEN,
        Keyword::ELSE,
        Keyword::CASE,
        Keyword::IS,
        Keyword::NULL,
        Keyword::ALL,
        Keyword::ANY,
        Keyword::SOME,
        Keyword::IN,
        Keyword::LIKE,
        Keyword::ILIKE,
        Keyword::BETWEEN,
        Keyword::HAVING,
        Keyword::RETURNING,
        Keyword::VALUES,
        Keyword::LIMIT,
        Keyword::OFFSET,
    ];
    match tokens.get(end)? {
        Token::Word(w) if w.quote_style.is_some() || !STRUCTURAL.contains(&w.keyword) => {
            Some(word_name(w))
        }
        _ => None,
    }
}

pub(crate) fn operand_before(tokens: &[Token], end: usize) -> Option<String> {
    match tokens.get(end)? {
        Token::RParen => column_before(tokens, end.checked_sub(1)?),
        _ => column_before(tokens, end),
    }
}

/// The column an operand starting at `start` names: `email` or `lower(email)`.
pub(crate) fn operand_after(tokens: &[Token], start: usize) -> Option<String> {
    if matches!(tokens.get(start)?, Token::Word(_)) && tokens.get(start + 1) == Some(&Token::LParen)
    {
        return column_before(tokens, start + 2);
    }
    column_before(tokens, start)
}

pub(crate) fn guess_name(tokens: &[Token], i: usize) -> Option<String> {
    // `$1` may be wrapped in a function call: `lower($1)`.
    let wrapped = tokens.get(i.wrapping_sub(1)) == Some(&Token::LParen)
        && matches!(tokens.get(i.wrapping_sub(2)), Some(Token::Word(w)) if w.keyword == Keyword::NoKeyword || w.keyword == Keyword::LOWER || w.keyword == Keyword::UPPER || w.keyword == Keyword::TRIM)
        && tokens.get(i + 1) == Some(&Token::RParen);
    let (start, end) = if wrapped { (i - 2, i + 1) } else { (i, i) };
    let prev = tokens.get(start.wrapping_sub(1));
    // col = $1, lower(col) = lower($1), col LIKE $1
    if is_comparison(prev)
        && let Some(name) = start.checked_sub(2).and_then(|k| operand_before(tokens, k))
    {
        return Some(name);
    }
    // $1 = col
    if is_comparison(tokens.get(end + 1))
        && let Some(name) = operand_after(tokens, end + 2)
    {
        return Some(name);
    }
    let prev = tokens.get(i.wrapping_sub(1));
    if is_keyword(prev, Keyword::LIMIT) {
        return Some("limit".to_owned());
    }
    if is_keyword(prev, Keyword::OFFSET) {
        return Some("offset".to_owned());
    }
    // col BETWEEN $1 AND $2
    if is_keyword(prev, Keyword::BETWEEN) {
        return column_before(tokens, i - 2).map(|c| format!("{c}_from"));
    }
    if is_keyword(prev, Keyword::AND)
        && placeholder(tokens.get(i.wrapping_sub(2))?).is_some()
        && is_keyword(tokens.get(i.wrapping_sub(3)), Keyword::BETWEEN)
    {
        return column_before(tokens, i - 4).map(|c| format!("{c}_to"));
    }
    // col = ANY($1), col IN ($1)
    if prev == Some(&Token::LParen) {
        let before = tokens.get(i.wrapping_sub(2));
        let column_at =
            if is_keyword(before, Keyword::ANY) && is_comparison(tokens.get(i.wrapping_sub(3))) {
                Some(i - 4)
            } else if is_keyword(before, Keyword::IN) {
                Some(i - 3)
            } else {
                None
            };
        if let Some(at) = column_at {
            return column_before(tokens, at).map(|c| naming::plural(&c));
        }
    }
    None
}

/// `INSERT INTO t (a, b) VALUES ($1, $2)`: name each parameter after its column.
pub(crate) fn insert_names(tokens: &[Token], mut i: usize, names: &mut [Option<String>]) {
    if tokens.get(i) != Some(&Token::LParen) {
        return;
    }
    i += 1;
    let mut columns = Vec::new();
    while let Some(t) = tokens.get(i) {
        match t {
            Token::Word(w) => columns.push(word_name(w)),
            Token::RParen => break,
            _ => {}
        }
        i += 1;
    }
    let Some(values) = (i..tokens.len()).find(|&j| is_keyword(tokens.get(j), Keyword::VALUES))
    else {
        return;
    };
    // Only the first row: ( item, item, ... ), where an item that is just `$n` is named.
    let mut j = values + 2;
    let mut position = 0;
    let mut depth = 0;
    let mut item: Vec<&Token> = Vec::new();
    while let Some(t) = tokens.get(j) {
        match t {
            Token::LParen => depth += 1,
            Token::RParen if depth == 0 => {
                assign(&item, position, &columns, names);
                break;
            }
            Token::RParen => depth -= 1,
            Token::Comma if depth == 0 => {
                assign(&item, position, &columns, names);
                item.clear();
                position += 1;
                j += 1;
                continue;
            }
            _ => {}
        }
        item.push(t);
        j += 1;
    }
}

pub(crate) fn assign(
    item: &[&Token],
    position: usize,
    columns: &[String],
    names: &mut [Option<String>],
) {
    if let ([t], Some(column)) = (item, columns.get(position))
        && let Some(n) = placeholder(t)
        && names[n - 1].is_none()
    {
        names[n - 1] = Some(column.clone());
    }
}
