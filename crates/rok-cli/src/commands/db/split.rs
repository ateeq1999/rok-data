//! Splitting a SQL script into its statements for `rok db query`.
//!
//! A `;` ends a statement only outside quoted text and comments, so the
//! scanner tracks PostgreSQL's lexical contexts: single-quoted strings
//! (with `''` and, in `E'…'` strings, backslash escapes), double-quoted
//! identifiers, dollar-quoted bodies (`$$…$$`, `$tag$…$tag$`), `--` line
//! comments and nesting `/* … */` block comments.

/// The statements of `sql`, in order, each trimmed and without its `;`.
///
/// A piece holding nothing but whitespace and comments is not a statement
/// and is left out, so a trailing comment or a doubled `;` sends nothing.
pub(super) fn statements(sql: &str) -> Vec<String> {
    let chars: Vec<char> = sql.chars().collect();
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut has_code = false;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();
        let end = match (ch, next) {
            ('-', Some('-')) => line_comment_end(&chars, i),
            ('/', Some('*')) => block_comment_end(&chars, i),
            ('\'', _) => {
                has_code = true;
                quoted_end(&chars, i, '\'', is_escape_string(&chars, i))
            }
            ('"', _) => {
                has_code = true;
                quoted_end(&chars, i, '"', false)
            }
            ('$', _) => {
                has_code = true;
                match dollar_tag(&chars, i) {
                    Some(tag) => dollar_body_end(&chars, i + tag.len(), &tag),
                    None => i + 1,
                }
            }
            (';', _) => {
                if has_code {
                    statements.push(current.trim().to_owned());
                }
                current.clear();
                has_code = false;
                i += 1;
                continue;
            }
            _ => {
                if !ch.is_whitespace() {
                    has_code = true;
                }
                i + 1
            }
        };
        current.extend(&chars[i..end]);
        i = end;
    }
    if has_code {
        statements.push(current.trim().to_owned());
    }
    statements
}

/// Index just past the newline ending the `--` comment that starts at `start`.
fn line_comment_end(chars: &[char], start: usize) -> usize {
    chars[start..]
        .iter()
        .position(|&c| c == '\n')
        .map_or(chars.len(), |offset| start + offset + 1)
}

/// Index just past the `*/` closing the block comment at `start`; block
/// comments nest in PostgreSQL.
fn block_comment_end(chars: &[char], start: usize) -> usize {
    let mut depth = 0usize;
    let mut i = start;
    while i < chars.len() {
        match (chars[i], chars.get(i + 1).copied()) {
            ('/', Some('*')) => {
                depth += 1;
                i += 2;
            }
            ('*', Some('/')) => {
                depth = depth.saturating_sub(1);
                i += 2;
                if depth == 0 {
                    return i;
                }
            }
            _ => i += 1,
        }
    }
    chars.len()
}

/// Index just past the quote closing the quoted text opened at `start`; a
/// doubled quote stays inside, and so does a backslash-escaped character
/// when `backslash_escapes` (an `E'…'` string).
fn quoted_end(chars: &[char], start: usize, quote: char, backslash_escapes: bool) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        let ch = chars[i];
        if backslash_escapes && ch == '\\' {
            i += 2;
        } else if ch == quote {
            if chars.get(i + 1) == Some(&quote) {
                i += 2;
            } else {
                return i + 1;
            }
        } else {
            i += 1;
        }
    }
    chars.len()
}

/// Whether the `'` at `quote` opens an escape string (`E'…'`): preceded by an
/// `E` that is not the tail of a longer word.
fn is_escape_string(chars: &[char], quote: usize) -> bool {
    quote >= 1
        && matches!(chars[quote - 1], 'E' | 'e')
        && (quote < 2 || !is_ident_char(chars[quote - 2]))
}

/// The dollar-quote tag (`$$` or `$name$`) opening at `start`, or `None`
/// for a `$` that opens no quote: a parameter (`$1`) or a `$` inside an
/// identifier (`a$b`).
fn dollar_tag(chars: &[char], start: usize) -> Option<String> {
    if start > 0 && is_ident_char(chars[start - 1]) {
        return None;
    }
    let mut i = start + 1;
    if let Some(&first) = chars.get(i)
        && (first.is_alphabetic() || first == '_')
    {
        i += 1;
        while chars
            .get(i)
            .is_some_and(|&c| c.is_alphanumeric() || c == '_')
        {
            i += 1;
        }
    }
    (chars.get(i) == Some(&'$')).then(|| chars[start..=i].iter().collect())
}

/// Index just past the closing `tag` of a dollar-quoted body starting at
/// `body` (the index after the opening tag).
fn dollar_body_end(chars: &[char], body: usize, tag: &str) -> usize {
    let tag: Vec<char> = tag.chars().collect();
    let mut i = body;
    while i + tag.len() <= chars.len() {
        if chars[i..i + tag.len()] == tag[..] {
            return i + tag.len();
        }
        i += 1;
    }
    chars.len()
}

/// A character that may continue an unquoted identifier.
fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

#[cfg(test)]
mod tests {
    use super::statements;

    #[test]
    fn semicolons_separate_statements() {
        assert_eq!(
            statements("CREATE TABLE t (id int); INSERT INTO t VALUES (1);"),
            ["CREATE TABLE t (id int)", "INSERT INTO t VALUES (1)"]
        );
    }

    #[test]
    fn a_last_statement_needs_no_semicolon() {
        assert_eq!(statements("SELECT 1; SELECT 2"), ["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn empty_pieces_and_comment_only_pieces_are_dropped() {
        assert_eq!(statements(" ;; SELECT 1;  -- done\n"), ["SELECT 1"]);
        assert!(statements("/* nothing */ ; -- here").is_empty());
    }

    #[test]
    fn a_semicolon_inside_a_string_stays() {
        assert_eq!(
            statements("SELECT 'a;b', 'it''s;'; SELECT 2"),
            ["SELECT 'a;b', 'it''s;'", "SELECT 2"]
        );
    }

    #[test]
    fn an_escape_string_backslash_quote_stays_inside() {
        assert_eq!(
            statements(r"SELECT E'x\';y'; SELECT 2"),
            [r"SELECT E'x\';y'", "SELECT 2"]
        );
    }

    #[test]
    fn a_backslash_in_a_plain_string_escapes_nothing() {
        assert_eq!(
            statements(r"SELECT 'x\'; SELECT 2"),
            [r"SELECT 'x\'", "SELECT 2"]
        );
    }

    #[test]
    fn a_semicolon_inside_a_quoted_identifier_stays() {
        assert_eq!(
            statements(r#"SELECT 1 AS "a;""b"; SELECT 2"#),
            [r#"SELECT 1 AS "a;""b""#, "SELECT 2"]
        );
    }

    #[test]
    fn a_dollar_quoted_body_keeps_its_semicolons() {
        let sql = "CREATE FUNCTION f() RETURNS int AS $$ BEGIN RETURN 1; END; $$ LANGUAGE plpgsql; SELECT f()";
        assert_eq!(
            statements(sql),
            [
                "CREATE FUNCTION f() RETURNS int AS $$ BEGIN RETURN 1; END; $$ LANGUAGE plpgsql",
                "SELECT f()"
            ]
        );
    }

    #[test]
    fn a_tagged_dollar_quote_ends_only_at_its_own_tag() {
        let sql = "SELECT $fn$ a; $$ b; $fn$; SELECT 2";
        assert_eq!(statements(sql), ["SELECT $fn$ a; $$ b; $fn$", "SELECT 2"]);
    }

    #[test]
    fn a_parameter_or_identifier_dollar_opens_no_quote() {
        assert_eq!(
            statements("SELECT a$b, $1; SELECT 2"),
            ["SELECT a$b, $1", "SELECT 2"]
        );
    }

    #[test]
    fn comments_hide_their_semicolons() {
        assert_eq!(
            statements("SELECT 1 -- a; b\n; SELECT /* c; /* nested; */ d; */ 2"),
            ["SELECT 1 -- a; b", "SELECT /* c; /* nested; */ d; */ 2"]
        );
    }

    #[test]
    fn an_unterminated_quote_runs_to_the_end() {
        assert_eq!(statements("SELECT 'a; b"), ["SELECT 'a; b"]);
    }
}
