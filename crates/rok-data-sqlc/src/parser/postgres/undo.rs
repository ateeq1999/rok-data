//! The `-- rok:down` undo statements of a schema input, skipped before the
//! SQL parser reads the text.
//!
//! A truth file may carry changes its `CREATE TABLE` statements do not
//! spell — an `ALTER TABLE books ADD COLUMN isbn`, and later such
//! statements — with the SQL that undoes each of them beside it. The
//! convention of the whole-crate generator (`rok-data-codegen::parse`) is a
//! `-- rok:down` comment line: the statement it sits above is the undo of
//! the statement immediately before the marker, and the generator keeps the
//! undo only for the migrations it writes. The SQL parser keeps no
//! comments, so without this pass the undo statement would be a statement
//! of its own as far as the schema parser is concerned, and it would undo
//! what the truth means to have.
//!
//! This pass scans the text like the codegen's statement splitter does
//! (`'…'` and `"…"` with doubled-quote escapes, dollar-quoted strings,
//! line and block comments; block comments do not nest), honors the
//! markers, and replaces every skip with spaces, newlines kept — the file's
//! line numbers, which its diagnostics carry, stay where they were.
//!
//! The marker is a `--` line comment whose whole content is `rok:down`
//! (the indent and the spaces after `--` are free): it must sit in the
//! whitespace between two statements — alone on a line, or right after the
//! previous statement's `;` — and it must be joined to the statement it
//! introduces, as the codegen's comment blocks are. The undo statement is
//! the one the marker introduces, up to its first top-level `;` (or the
//! end of the text); the statement before the marker stays, and everything
//! else in the file is left to the SQL parser untouched.
//!
//! Where the codegen parses the same text, the decisions agree, except
//! where a silent codegen outcome would be a wrong one here — the codegen
//! loses a marker that lost its comment block and drops a trailing marker
//! comment without a word, and the engine cannot: applying the statement
//! below would be the wrong half of the pair. So:
//!
//! - a marker with no statement to undo (the file's first statement), or
//!   one whose previous statement was itself an undo, errors exactly as
//!   the codegen's parse does;
//! - a marker with no undo statement after it, or one that lost its
//!   statement across a blank line (or to an empty statement), errors with
//!   the codegen's rule told from the marker's side; a marker line inside
//!   a statement (a string literal for one) was never a marker, in either
//!   tool.
//!
//! Errors point at the marker's line, in the shape every schema error of
//! this parser points (`path:line:`; `line N:` when no path is known).
//!
//! The decision: this crate's `docs/adr/0017-truth-input-rok-down-markers.md`.

use std::borrow::Cow;
use std::path::Path;

use anyhow::Result;

use crate::report::display_path;

/// Skips the `-- rok:down` undo statements of a schema input: the text to
/// parse comes back either as it was (no marker in it) or with every skip
/// a span of spaces, so the parser's spans still name the file's own lines
/// (see the module docs).
pub(crate) fn strip_undos<'a>(sql: &'a str, path: &Path) -> Result<Cow<'a, str>> {
    let mut scan = Scan::new(sql, path);
    let skips = scan.run()?;
    if skips.is_empty() {
        return Ok(Cow::Borrowed(sql));
    }
    let mut out = String::new();
    let mut cursor = 0;
    for (start, end) in skips {
        out.push_str(&sql[cursor..start]);
        out.extend(
            sql[start..end]
                .chars()
                .map(|c| if c == '\n' { '\n' } else { ' ' }),
        );
        cursor = end;
    }
    out.push_str(&sql[cursor..]);
    Ok(Cow::Owned(out))
}

/// The error of a marker that promises an undo nothing brings: the codegen's
/// rule, told from the marker's side.
fn lost_undo(path: &Path, line: usize) -> anyhow::Error {
    error_at(
        path,
        line,
        "`-- rok:down` must be followed by the SQL that undoes the statement above it",
    )
}

/// A comment line's content is this, whitespace aside, to be a marker.
const MARKER: &[u8] = b"rok:down";

/// The scan of one schema input: where the markers are, and the skips to
/// blank out of the text.
struct Scan<'a> {
    text: &'a [u8],
    path: &'a Path,
    /// 1-based line of the scan position.
    line: usize,
    /// The marker that awaits its statement: its line (for the errors) and
    /// where its comment begins (a skip starts there).
    marker: Option<(usize, usize)>,
    /// Whether statement content runs under the scan, since the last `;`.
    statement: bool,
    /// Whether any statement was seen in the file at all.
    seen: bool,
    /// Whether the previous statement was skipped as an undo, so a next
    /// marker has no statement to undo.
    after_undo: bool,
    /// Where the statement under the marker began, when it began.
    undo_start: Option<usize>,
    /// The skips collected so far, in order.
    skips: Vec<(usize, usize)>,
}

impl<'a> Scan<'a> {
    fn new(sql: &'a str, path: &'a Path) -> Self {
        Self {
            text: sql.as_bytes(),
            path,
            line: 1,
            marker: None,
            statement: false,
            seen: false,
            after_undo: false,
            undo_start: None,
            skips: Vec::new(),
        }
    }

    fn run(&mut self) -> Result<Vec<(usize, usize)>> {
        let len = self.text.len();
        let mut i = 0;
        while i < len {
            match self.text[i] {
                b'\n' => {
                    self.line += 1;
                    i += 1;
                    if let Some((line, _)) = self.marker {
                        // Only while the statement under the marker has
                        // not begun: a blank line inside the undo
                        // statement is its own whitespace.
                        if !self.statement && self.line_is_blank(i) {
                            // The marker lost its comment block (the codegen's
                            // split ends it there): the undo never came.
                            self.marker = None;
                            return Err(lost_undo(self.path, line));
                        }
                    }
                }
                b'-' if self.text.get(i + 1) == Some(&b'-') => {
                    let end = line_comment_end(self.text, i);
                    self.read_marker(i, end)?;
                    i = end;
                }
                b'/' if self.text.get(i + 1) == Some(&b'*') => {
                    let end = block_comment_end(self.text, i);
                    self.count_lines(i, end);
                    i = end;
                }
                quote @ (b'\'' | b'"') => {
                    let end = quoted_end(self.text, i, quote);
                    self.count_lines(i, end);
                    i = end;
                }
                b'$' => {
                    let end = dollar_quote_end(self.text, i).unwrap_or(i + 1);
                    self.count_lines(i, end);
                    i = end;
                }
                b';' => {
                    i += 1;
                    let marker = self.marker.take();
                    let undo_start = self.undo_start.take();
                    match (marker, undo_start) {
                        // The statement that just ended is the undo. The
                        // skip runs over the marker's comment and the
                        // statement, up to the end of the `;`.
                        (Some((_, start)), Some(_)) => {
                            self.skips.push((start, i));
                            self.statement = false;
                            self.after_undo = true;
                        }
                        // An empty statement ate the marker's block: the
                        // undo did not begin where the marker promised.
                        (Some((line, _)), None) => return Err(lost_undo(self.path, line)),
                        // A statement of its own ended.
                        (None, _) if self.statement => {
                            self.statement = false;
                            self.after_undo = false;
                        }
                        // A `;` with nothing on it ends nothing.
                        (None, _) => {}
                    }
                }
                byte => {
                    if !byte.is_ascii_whitespace() && !self.statement {
                        // First statement content: the statement under the
                        // marker begins here, and a statement of its own
                        // stands.
                        self.statement = true;
                        self.seen = true;
                        self.after_undo = false;
                        if self.marker.is_some() {
                            self.undo_start = Some(i);
                        }
                    }
                    i += 1;
                }
            }
        }
        match (self.marker.take(), self.undo_start.take()) {
            // The undo statement ran to the end of the file: still the
            // undo (the codegen's split keeps a trailing statement).
            (Some((_, start)), Some(_)) => self.skips.push((start, len)),
            (Some((line, _)), None) => return Err(lost_undo(self.path, line)),
            (None, _) => {}
        }
        Ok(std::mem::take(&mut self.skips))
    }

    /// Reads the line comment from `i` (`--`) to `end` (the end of line)
    /// as a marker: only in the whitespace between statements, where a
    /// marker lives (the codegen reads a directive only from a comment
    /// block, and a statement's own comments are its text).
    fn read_marker(&mut self, i: usize, end: usize) -> Result<()> {
        if self.statement || !self.is_marker(i, end) {
            return Ok(());
        }
        if self.marker.is_some() {
            // Another marker line above the same statement: the codegen's
            // directive reader sees both as the same marker.
            return Ok(());
        }
        if !self.seen || self.after_undo {
            return Err(error_at(
                self.path,
                self.line,
                "`-- rok:down` must directly follow the statement it undoes",
            ));
        }
        self.marker = Some((self.line, i));
        Ok(())
    }

    /// Whether the content of the line comment from `i` (`--`) to `end`
    /// (the end of line) is a marker, the codegen's way: surrounding
    /// whitespace aside, exactly `rok:down`.
    fn is_marker(&self, i: usize, end: usize) -> bool {
        trimmed(&self.text[i + 2..end]) == MARKER
    }

    /// The line at `i` holds no content but whitespace (or nothing is left
    /// at all). Only asked while a marker awaits its statement.
    fn line_is_blank(&self, i: usize) -> bool {
        let mut j = i;
        while j < self.text.len() && self.text[j] != b'\n' && self.text[j].is_ascii_whitespace() {
            j += 1;
        }
        j >= self.text.len() || self.text[j] == b'\n'
    }

    /// Counts the newlines a skipped token spans (`i..end`), both within
    /// the text.
    fn count_lines(&mut self, start: usize, end: usize) {
        self.line += self.text[start..end]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count();
    }
}

/// A schema error of the pre-pass, at the marker's line: the same shape
/// every syntax error of this parser points (`path:line:`; when no path is
/// known, `line N:`).
fn error_at(path: &Path, line: usize, message: &str) -> anyhow::Error {
    anyhow::Error::msg(if path.as_os_str().is_empty() {
        format!("line {line}: {message}")
    } else {
        format!("{}:{line}: {message}", display_path(path))
    })
}

/// The end of the line comment at `--` (`i`), before its `\n`.
fn line_comment_end(text: &[u8], i: usize) -> usize {
    let mut end = i + 2;
    while end < text.len() && text[end] != b'\n' {
        end += 1;
    }
    end
}

/// The end of the block comment at `/*` (`i`): the first `*/`, as the
/// codegen's splitter scans them (they do not nest), or the text's end.
fn block_comment_end(text: &[u8], i: usize) -> usize {
    let mut end = i + 2;
    let mut closing = None;
    while let Some(&byte) = text.get(end) {
        if byte == b'*' && text.get(end + 1) == Some(&b'/') {
            closing = Some(end + 2);
            break;
        }
        end += 1;
    }
    closing.unwrap_or(text.len())
}

/// The end of the quoted text at `i` (one past its closing `quote`), where
/// a doubled quote is an escaped quote, as the codegen's splitter reads
/// them. The text's end when the quote never closes.
fn quoted_end(text: &[u8], i: usize, quote: u8) -> usize {
    let mut end = i + 1;
    while let Some(&byte) = text.get(end) {
        if byte == quote {
            if text.get(end + 1) == Some(&quote) {
                end += 2;
                continue;
            }
            return end + 1;
        }
        end += 1;
    }
    text.len()
}

/// The end of the `$tag$ string $tag$` starting at `i`, or `None` where
/// `$` starts something else (a `$1` parameter), as the codegen's splitter
/// reads them. The text's end when the closing tag never comes.
fn dollar_quote_end(text: &[u8], i: usize) -> Option<usize> {
    let mut close = i + 1;
    while close < text.len()
        && (text[close].is_ascii_alphanumeric() || text[close] == b'_' || text[close] >= 0x80)
    {
        close += 1;
    }
    if close >= text.len() || text[close] != b'$' || text.get(i + 1).is_some_and(u8::is_ascii_digit)
    {
        return None;
    }
    let tag = &text[i..=close];
    let mut end = close + 1;
    while end + tag.len() <= text.len() {
        if &text[end..end + tag.len()] == tag {
            return Some(end + tag.len());
        }
        end += 1;
    }
    Some(text.len())
}

/// The slice without its surrounding whitespace.
fn trimmed(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(0, |position| position + 1);
    &bytes[start..end]
}
