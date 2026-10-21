//! The parser's own tests.

use super::*;

fn parse_text(text: &str) -> Result<Parsed, Error> {
    parse(&[SourceFile {
        path: "db/user.sql".into(),
        module: "user".into(),
        text: text.into(),
    }])
}

#[test]
fn numeric_and_interval_spellings() {
    let p = parse_text(
        "CREATE TABLE plans (
                 price NUMERIC(10, 2) NOT NULL,
                 rate DECIMAL,
                 period INTERVAL NOT NULL,
                 grace INTERVAL DAY TO SECOND,
                 timeout INTERVAL(3),
                 tiers NUMERIC[]
             );",
    )
    .unwrap();
    let types: Vec<&str> = p.schema.tables[0]
        .columns
        .iter()
        .map(|c| c.sql_type.as_str())
        .collect();
    assert_eq!(
        types,
        [
            "NUMERIC(10,2)",
            "NUMERIC",
            "INTERVAL",
            "INTERVAL DAY TO SECOND",
            "INTERVAL(3)",
            "NUMERIC[]"
        ]
    );
}

#[test]
fn tables_columns_and_constraints() {
    let p = parse_text(
        "CREATE TYPE mood AS ENUM ('happy', 'sad');
             -- A user.
             CREATE TABLE users (
                 id BIGSERIAL PRIMARY KEY,
                 -- Where we write.
                 email VARCHAR(255) NOT NULL UNIQUE,
                 manager_id BIGINT REFERENCES users (id) ON DELETE SET NULL,
                 mood mood NOT NULL DEFAULT 'happy',
                 -- rok:tenant
                 org_id INT NOT NULL,
                 lower_email TEXT GENERATED ALWAYS AS (lower(email)) STORED,
                 CHECK (org_id > 0)
             );
             CREATE INDEX users_org_idx ON users (org_id);
             -- name: find_by_email :one
             SELECT * FROM users WHERE email = $1;",
    )
    .unwrap();
    let t = &p.schema.tables[0];
    assert_eq!(t.doc, ["A user."]);
    assert_eq!(t.primary_key, ["id"]);
    let email = t.column("email").unwrap();
    assert_eq!(
        (email.sql_type.as_str(), email.nullable),
        ("VARCHAR(255)", false)
    );
    assert_eq!(email.doc, ["Where we write."]);
    assert_eq!(t.uniques[0].name, "users_email_key");
    assert_eq!(t.foreign_keys[0].name, "users_manager_id_fkey");
    assert_eq!(t.foreign_keys[0].on_delete.as_deref(), Some("SET NULL"));
    assert_eq!(
        t.column("mood").unwrap().default.as_deref(),
        Some("'happy'")
    );
    assert!(t.column("org_id").unwrap().tenant);
    assert_eq!(
        t.column("lower_email").unwrap().generated.as_deref(),
        Some("lower(email)")
    );
    assert_eq!(t.checks[0].name, "users_check");
    assert_eq!(p.schema.enums[0].values, ["happy", "sad"]);
    assert_eq!(p.schema.indexes[0].name, "users_org_idx");
    assert_eq!(p.queries[0].name, "find_by_email");
    assert_eq!(p.queries[0].kind, QueryKind::One);
}

#[test]
fn extras_need_a_down() {
    let err =
        parse_text("CREATE FUNCTION f() RETURNS int AS $$ SELECT 1 $$ LANGUAGE sql;").unwrap_err();
    assert!(err.to_string().contains("rok:down"), "{err}");
    let p = parse_text(
        "CREATE FUNCTION f() RETURNS int AS $$ SELECT 1 $$ LANGUAGE sql;
             -- rok:down
             DROP FUNCTION f();",
    )
    .unwrap();
    assert_eq!(p.schema.extras[0].down, "DROP FUNCTION f()");
}

/// An `ALTER TABLE` changes the parsed table itself: the fold needs no
/// `-- rok:down`, and the later `CREATE TABLE` describing the folded shape
/// parses identically (pinned end-to-end by
/// `an_alter_folded_into_the_table_stages_nothing` in the diff's tests).
#[test]
fn alters_change_the_tables() {
    let p = parse_text(
        "CREATE TABLE books (id BIGSERIAL PRIMARY KEY, title TEXT NOT NULL);
             ALTER TABLE books
                 ADD COLUMN isbn TEXT UNIQUE,
                 ADD COLUMN year INT NOT NULL DEFAULT 0,
                 DROP COLUMN title,
                 ALTER COLUMN isbn SET DATA TYPE VARCHAR(64);",
    )
    .unwrap();
    let table = &p.schema.tables[0];
    assert!(p.schema.extras.is_empty());
    assert_eq!(
        table
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "isbn", "year"]
    );
    let isbn = table.column("isbn").unwrap();
    assert_eq!(isbn.sql_type, "VARCHAR(64)");
    assert!(isbn.nullable);
    let year = table.column("year").unwrap();
    assert!(!year.nullable && year.default.as_deref() == Some("0"));
    assert_eq!(table.uniques[0].name, "books_isbn_key");

    // A rename follows the column into the keys that name it.
    let p = parse_text(
        "CREATE TABLE teams (id BIGSERIAL PRIMARY KEY, leader_id BIGINT REFERENCES teams, CONSTRAINT teams_leader_key UNIQUE (leader_id));
             ALTER TABLE teams RENAME COLUMN leader_id TO captain_id;",
    )
    .unwrap();
    let table = &p.schema.tables[0];
    assert_eq!(table.foreign_keys[0].columns, ["captain_id"]);
    assert_eq!(table.uniques[0].columns, ["captain_id"]);

    // An `ADD CONSTRAINT` folds the same way a `CREATE TABLE` one would.
    let p = parse_text(
        "CREATE TABLE shelves (room_id INT, row INT);
             ALTER TABLE shelves ADD CONSTRAINT shelves_pkey PRIMARY KEY (room_id, row);",
    )
    .unwrap();
    assert_eq!(p.schema.tables[0].primary_key, ["room_id", "row"]);
    assert!(!p.schema.tables[0].column("room_id").unwrap().nullable);
}

/// A clause the model cannot represent keeps the statement on the extras
/// path: without `-- rok:down` it is refused, with one it rides as
/// written.
#[test]
fn unsupported_alter_clauses_still_need_a_down() {
    let err = parse_text(
        "CREATE TABLE books (id BIGSERIAL PRIMARY KEY);
             ALTER TABLE books SET UNLOGGED;",
    )
    .unwrap_err();
    assert!(err.to_string().contains("rok can't undo"), "{err}");
    let p = parse_text(
        "CREATE TABLE books (id BIGSERIAL PRIMARY KEY);
             ALTER TABLE books SET UNLOGGED;
             -- rok:down
             ALTER TABLE books SET LOGGED;",
    )
    .unwrap();
    assert_eq!(p.schema.extras[0].up, "ALTER TABLE books SET UNLOGGED");
}

#[test]
fn errors_point_at_the_file() {
    let err = parse_text("CREATE TABLE posts (id INT PRIMARY KEY, user_id INT REFERENCES users);")
        .unwrap_err();
    assert!(err.to_string().contains("users"), "{err}");
    let err = parse_text("\n-- name: x :maybe\nSELECT 1;").unwrap_err();
    assert!(err.to_string().contains("db/user.sql:3"), "{err}");
}
