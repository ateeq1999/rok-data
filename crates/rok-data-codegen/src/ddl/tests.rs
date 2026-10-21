//! The migration diff's own tests.

use super::*;
use crate::parse::{SourceFile, parse};

fn schema(text: &str) -> Schema {
    parse(&[SourceFile {
        path: "db/app.sql".into(),
        module: "app".into(),
        text: text.into(),
    }])
    .unwrap()
    .schema
}

const V1: &str = "
        CREATE TYPE role AS ENUM ('admin', 'member');
        CREATE TABLE users (id BIGSERIAL PRIMARY KEY, email TEXT NOT NULL UNIQUE, role role NOT NULL DEFAULT 'member');
        CREATE TABLE posts (id BIGSERIAL PRIMARY KEY, user_id BIGINT NOT NULL REFERENCES users, title TEXT NOT NULL);
        CREATE INDEX posts_user_idx ON posts (user_id);";

#[test]
fn init_creates_everything_and_undoes_it() {
    let ops = diff(&Schema::default(), &schema(V1), &[]).unwrap();
    let (up, down) = scripts(&ops);
    let create_users = up.find("CREATE TABLE \"users\"").unwrap();
    let fk = up
        .find("FOREIGN KEY (\"user_id\") REFERENCES \"users\" (\"id\")")
        .unwrap();
    assert!(up.starts_with("CREATE TYPE \"role\" AS ENUM ('admin', 'member');"));
    assert!(create_users < fk, "{up}");
    assert!(
        up.contains("\"role\" \"role\" NOT NULL DEFAULT 'member'"),
        "{up}"
    );
    assert!(
        up.ends_with("CREATE INDEX posts_user_idx ON posts(user_id);\n"),
        "{up}"
    );
    assert!(
        down.starts_with("DROP INDEX IF EXISTS \"posts_user_idx\";"),
        "{down}"
    );
    assert!(down.ends_with("DROP TYPE \"role\";\n"), "{down}");
    assert!(ops.iter().all(|op| op.destructive.is_none()));
}

#[test]
fn no_changes_no_steps() {
    assert!(diff(&schema(V1), &schema(V1), &[]).unwrap().is_empty());
}

#[test]
fn column_and_enum_changes() {
    let v2 = V1
        .replace("'admin', 'member'", "'admin', 'editor', 'member'")
        .replace(
            "title TEXT NOT NULL",
            "title TEXT, slug TEXT NOT NULL DEFAULT ''",
        );
    let ops = diff(&schema(V1), &schema(&v2), &[]).unwrap();
    let ups: Vec<&str> = ops.iter().map(|op| op.up.as_str()).collect();
    assert_eq!(
        ups,
        [
            "ALTER TYPE \"role\" ADD VALUE 'editor' AFTER 'admin'",
            "ALTER TABLE \"posts\" ALTER COLUMN \"title\" DROP NOT NULL",
            "ALTER TABLE \"posts\" ADD COLUMN \"slug\" TEXT NOT NULL DEFAULT ''",
        ]
    );
    // Adding an enum value can't be undone.
    assert!(scripts(&ops).1.is_empty());
}

#[test]
fn destructive_steps_are_flagged() {
    let v2 = V1.replace(", title TEXT NOT NULL", ", body TEXT NOT NULL");
    let ops = diff(&schema(V1), &schema(&v2), &[]).unwrap();
    let reasons: Vec<&str> = ops
        .iter()
        .filter_map(|op| op.destructive.as_deref())
        .collect();
    assert_eq!(reasons.len(), 2, "{reasons:?}");
    assert!(reasons[0].contains("drops column `posts.title`"));
    assert!(reasons[1].contains("without a DEFAULT"));

    // ...unless it is a rename.
    let rename = [Rename::parse("posts.title=body").unwrap()];
    let ops = diff(&schema(V1), &schema(&v2), &rename).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(
        ops[0].up,
        "ALTER TABLE \"posts\" RENAME COLUMN \"title\" TO \"body\""
    );
}

#[test]
fn removed_enum_values_are_refused() {
    let v2 = V1.replace("'admin', 'member'", "'member'");
    assert!(
        diff(&schema(V1), &schema(&v2), &[])
            .unwrap_err()
            .contains("lost or reordered")
    );
}

/// The plan's staging experiment, pinned: a previous state whose
/// `ALTER TABLE` rides as an `Extra` (the truth's `-- rok:down` supplied
/// its undo), and a new state with the column folded into the `CREATE
/// TABLE`, stage a pair whose up starts with the removed `Extra`'s own
/// undo before the added column — the redundant drop-and-add instead of
/// leaving the shape alone (`docs/unified-pipeline-plan.md`'s Open note).
#[test]
fn an_alter_folded_into_the_table_stages_drop_then_add() {
    let old = schema(
        "CREATE TABLE books (id BIGSERIAL PRIMARY KEY, title TEXT NOT NULL);
        ALTER TABLE books ADD COLUMN isbn TEXT;
        -- rok:down
        ALTER TABLE books DROP COLUMN isbn;",
    );
    assert_eq!(old.extras.len(), 1, "the ALTER rides as an `Extra`");
    assert_eq!(old.extras[0].up, "ALTER TABLE books ADD COLUMN isbn TEXT");
    assert_eq!(old.extras[0].down, "ALTER TABLE books DROP COLUMN isbn");
    assert!(!old.extras[0].before_tables);

    let new =
        schema("CREATE TABLE books (id BIGSERIAL PRIMARY KEY, title TEXT NOT NULL, isbn TEXT);");
    let ops = diff(&old, &new, &[]).unwrap();
    assert_eq!(
        ops.iter().map(|op| op.up.as_str()).collect::<Vec<_>>(),
        [
            // The removed `Extra`: its undo (the marker supplied it) runs
            // first, its up step being the changed pair's down.
            "ALTER TABLE books DROP COLUMN isbn",
            // Then the column, folded into the truth's `CREATE TABLE`.
            "ALTER TABLE \"books\" ADD COLUMN \"isbn\" TEXT",
        ]
    );
    assert!(ops.iter().all(|op| op.destructive.is_none()));
    let (up, down) = scripts(&ops);
    assert!(
        up.starts_with("ALTER TABLE books DROP COLUMN isbn;\n"),
        "{up}"
    );
    assert!(
        down.starts_with("ALTER TABLE \"books\" DROP COLUMN \"isbn\";\n"),
        "{down}"
    );
}
