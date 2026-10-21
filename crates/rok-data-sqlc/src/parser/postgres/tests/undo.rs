//! The `-- rok:down` markers of a truth file: the undo statement they
//! introduce is skipped, and a misplaced marker is refused, like the
//! whole-crate generator's parse does (see `postgres::undo`).

use std::borrow::Cow;

use super::*;
use crate::parser::postgres::undo::strip_undos;

#[test]
fn adds_the_column_and_skips_the_undo_its_marker_marks() {
    let parsed = parse(
        "CREATE TABLE books (id BIGSERIAL PRIMARY KEY, title TEXT NOT NULL);
             ALTER TABLE books ADD COLUMN isbn TEXT;
             -- rok:down
             ALTER TABLE books DROP COLUMN isbn;

             CREATE TABLE shelves (id BIGINT PRIMARY KEY, book_id BIGINT NOT NULL REFERENCES books (id));",
    );
    assert!(parsed.failures.is_empty(), "{:?}", parsed.failures);
    assert_eq!(parsed.schema.tables.len(), 2);
    let books = &parsed.schema.tables[0];
    assert_eq!(books.sql_name, "books");
    assert_eq!(column_names(books), ["id", "title", "isbn"]);
    let shelves = &parsed.schema.tables[1];
    assert_eq!(column_names(shelves), ["id", "book_id"]);
}

#[test]
fn the_undo_may_run_to_the_end_of_the_file() {
    let parsed = parse(
        "CREATE TABLE books (id BIGSERIAL PRIMARY KEY, title TEXT NOT NULL);
             ALTER TABLE books ADD COLUMN isbn TEXT;
             -- rok:down
             ALTER TABLE books DROP COLUMN isbn",
    );
    assert!(parsed.failures.is_empty(), "{:?}", parsed.failures);
    assert_eq!(
        column_names(&parsed.schema.tables[0]),
        ["id", "title", "isbn"]
    );
}

#[test]
fn a_marker_without_a_statement_to_undo_is_refused() {
    let error = parse_schema("-- rok:down\n\nALTER TABLE books DROP COLUMN isbn;")
        .unwrap_err()
        .to_string();
    // The generator's own rule wording, for a marker whose file holds no
    // statement before it.
    assert!(
        error.contains("`-- rok:down` must directly follow the statement it undoes"),
        "{error}"
    );
}

#[test]
fn a_file_s_marker_has_no_statement_of_an_earlier_file_before_it() {
    let error = parse_sources(&[
        SchemaSource {
            path: Path::new("0001_init.sql"),
            sql: "CREATE TABLE books (id BIGSERIAL PRIMARY KEY);",
        },
        SchemaSource {
            path: Path::new("0002_loans.sql"),
            sql: "-- rok:down\nALTER TABLE books DROP COLUMN isbn;",
        },
    ])
    .unwrap_err()
    .to_string();
    assert!(
        error.contains(
            "0002_loans.sql:1: `-- rok:down` must directly follow the statement it undoes"
        ),
        "{error}"
    );
}

#[test]
fn a_marker_cannot_follow_another_marked_undo() {
    let error = parse_schema(
        "ALTER TABLE books ADD COLUMN isbn TEXT;
             -- rok:down
             ALTER TABLE books DROP COLUMN isbn;
             -- rok:down
             ALTER TABLE books DROP COLUMN isbn;",
    )
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("`-- rok:down` must directly follow the statement it undoes"),
        "{error}"
    );
}

#[test]
fn a_marker_without_an_undo_statement_is_refused() {
    let error = parse_schema("ALTER TABLE books ADD COLUMN isbn TEXT;\n-- rok:down\n")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains(
            "line 2: `-- rok:down` must be followed by the SQL that undoes the statement above it"
        ),
        "{error}"
    );
}

#[test]
fn the_marker_s_statement_must_come_right_after_it() {
    // A blank line ends the marker's comment block (the codegen loses the
    // marker silently there, and applies the undo as a statement): the
    // parser refuses instead.
    let error = parse_schema(
        "ALTER TABLE books ADD COLUMN isbn TEXT;
             -- rok:down

             ALTER TABLE books DROP COLUMN isbn;",
    )
    .unwrap_err()
    .to_string();
    assert!(
        error.contains(
            "line 2: `-- rok:down` must be followed by the SQL that undoes the statement above it"
        ),
        "{error}"
    );
}

#[test]
fn an_ordinary_comment_does_nothing() {
    let parsed = parse(
        "-- the books every member may borrow\nCREATE TABLE books (id BIGSERIAL PRIMARY KEY, title TEXT NOT NULL);\n-- the isbn is added here, not in the table\nALTER TABLE books ADD COLUMN isbn TEXT;",
    );
    assert!(parsed.failures.is_empty(), "{:?}", parsed.failures);
    assert_eq!(
        column_names(&parsed.schema.tables[0]),
        ["id", "title", "isbn"]
    );
}

#[test]
fn a_comment_beside_the_marker_does_not_move_it() {
    let parsed = parse(
        "CREATE TABLE books (id BIGSERIAL PRIMARY KEY, title TEXT NOT NULL);
             ALTER TABLE books ADD COLUMN isbn TEXT;
             -- the isbn is the truth's change
             -- rok:down
             ALTER TABLE books DROP COLUMN isbn;
             -- the next migration says what it staged",
    );
    assert!(parsed.failures.is_empty(), "{:?}", parsed.failures);
    assert_eq!(
        column_names(&parsed.schema.tables[0]),
        ["id", "title", "isbn"]
    );
}

#[test]
fn a_marker_inside_a_string_literal_is_its_text() {
    // Line-based markers stay line-based: the codegen reads a directive
    // only from a comment block, and a literal's line is its content.
    let parsed = parse(
        "CREATE TABLE books (id BIGSERIAL PRIMARY KEY, note TEXT DEFAULT 'write
-- rok:down
their undo here');",
    );
    assert!(parsed.failures.is_empty(), "{:?}", parsed.failures);
    assert_eq!(column_names(&parsed.schema.tables[0]), ["id", "note"]);
}

#[test]
fn a_marker_inside_a_dollar_quoted_body_is_its_text() {
    let parsed = parse(
        "CREATE TABLE books (id BIGSERIAL PRIMARY KEY);\nINSERT INTO books VALUES (1, $$-- rok:down$$);",
    );
    assert!(parsed.failures.is_empty(), "{:?}", parsed.failures);
    assert_eq!(column_names(&parsed.schema.tables[0]), ["id"]);
}

#[test]
fn the_error_points_at_the_marker_s_line_of_its_file() {
    let sources = [SchemaSource {
        path: Path::new("schema/books.sql"),
        sql: "ALTER TABLE books ADD COLUMN isbn TEXT;
-- rok:down
ALTER TABLE books DROP COLUMN isbn;
-- rok:down
ALTER TABLE books DROP COLUMN isbn;",
    }];
    let error = parse_sources(&sources).unwrap_err().to_string();
    assert!(error.contains("schema/books.sql:4: "), "{error}");
}

#[test]
fn the_skip_leaves_the_lines_where_they_were() {
    let sql = "CREATE TABLE books (id INT);\n-- rok:down\nALTER TABLE books DROP COLUMN isbn;\nSELECT 'x';\n";
    let filtered = strip_undos(sql, Path::new("")).unwrap();
    assert!(
        matches!(filtered, Cow::Owned(_)),
        "the skips turn the text into a filtered copy"
    );
    let expected = format!(
        "CREATE TABLE books (id INT);\n{}\n{}\nSELECT 'x';\n",
        " ".repeat("-- rok:down".len()),
        " ".repeat("ALTER TABLE books DROP COLUMN isbn;".len()),
    );
    assert_eq!(&*filtered, expected);
    let clean = "CREATE TABLE books (id INT);\n";
    assert!(
        matches!(strip_undos(clean, Path::new("")).unwrap(), Cow::Borrowed(_)),
        "a file without a marker parses its own text"
    );
}
