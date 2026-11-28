//! Replacing a changed function or trigger in place.

use crate::ir::Schema;
use crate::parse::{SourceFile, parse};
use crate::truth::migration;

fn schema(text: &str) -> Schema {
    parse(&[SourceFile {
        path: "db/app.sql".into(),
        module: "app".into(),
        text: text.into(),
    }])
    .expect("the schema parses")
    .schema
}

/// `touch_updated_at()` with the given body line, used by three triggers.
fn touched(body: &str) -> String {
    let mut text = format!(
        "CREATE FUNCTION touch_updated_at() RETURNS trigger AS $$
BEGIN
    {body}
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
-- rok:down
DROP FUNCTION touch_updated_at();
"
    );
    for table in ["users", "posts", "tags"] {
        text.push_str(&format!(
            "
CREATE TABLE {table} (id BIGINT PRIMARY KEY, updated_at TIMESTAMPTZ NOT NULL DEFAULT now());

CREATE TRIGGER {table}_touch BEFORE UPDATE ON {table}
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
-- rok:down
DROP TRIGGER {table}_touch ON {table};
"
        ));
    }
    text
}

#[test]
fn a_changed_function_body_is_replaced_in_place() {
    let old = schema(&touched("NEW.updated_at = now();"));
    let new = schema(&touched("NEW.updated_at = clock_timestamp();"));

    let pair = migration(&old, &new, &[]).expect("the change stages");
    assert_eq!(pair.steps.len(), 1, "{}", pair.up);
    assert!(
        pair.up
            .starts_with("CREATE OR REPLACE FUNCTION touch_updated_at()"),
        "{}",
        pair.up
    );
    assert!(pair.up.contains("clock_timestamp()"), "{}", pair.up);
    assert!(!pair.up.contains("DROP"), "{}", pair.up);
    assert!(
        pair.down
            .starts_with("CREATE OR REPLACE FUNCTION touch_updated_at()")
            && pair.down.contains("NEW.updated_at = now();"),
        "the undo restores the old body: {}",
        pair.down
    );
    assert!(pair.destructive.is_empty());
}

#[test]
fn a_changed_trigger_is_replaced_in_place() {
    let old = schema(&touched("NEW.updated_at = now();"));
    let new = schema(&touched("NEW.updated_at = now();").replace(
        "CREATE TRIGGER posts_touch BEFORE UPDATE ON posts",
        "CREATE TRIGGER posts_touch BEFORE UPDATE OF updated_at ON posts",
    ));

    let pair = migration(&old, &new, &[]).expect("the change stages");
    assert_eq!(pair.steps.len(), 1, "{}", pair.up);
    assert!(
        pair.up
            .starts_with("CREATE OR REPLACE TRIGGER posts_touch BEFORE UPDATE OF updated_at"),
        "{}",
        pair.up
    );
    assert!(!pair.up.contains("DROP"), "{}", pair.up);
    assert!(
        pair.down
            .starts_with("CREATE OR REPLACE TRIGGER posts_touch BEFORE UPDATE ON posts"),
        "{}",
        pair.down
    );
}

#[test]
fn a_changed_signature_is_refused_naming_the_triggers() {
    let old = schema(&touched("NEW.updated_at = now();"));
    let new = schema(
        &touched("NEW.updated_at = now();")
            .replace(
                "touch_updated_at() RETURNS",
                "touch_updated_at(stamp TEXT) RETURNS",
            )
            .replace(
                "DROP FUNCTION touch_updated_at()",
                "DROP FUNCTION touch_updated_at(TEXT)",
            ),
    );

    let refused = migration(&old, &new, &[])
        .expect_err("a changed signature can't be replaced")
        .to_string();
    assert!(
        refused.contains("touch_updated_at()") && refused.contains("touch_updated_at(text)"),
        "{refused}"
    );
    for trigger in ["users_touch", "posts_touch", "tags_touch"] {
        assert!(refused.contains(trigger), "{refused}");
    }
    assert!(refused.contains("hand-made migration"), "{refused}");
}

/// A new overload beside an unchanged function is no change of signature.
#[test]
fn a_new_overload_is_added_beside_the_old_one() {
    let old = schema(&touched("NEW.updated_at = now();"));
    let new = schema(&format!(
        "{}
CREATE FUNCTION touch_updated_at(stamp TEXT) RETURNS text AS $$ SELECT stamp $$ LANGUAGE sql;
-- rok:down
DROP FUNCTION touch_updated_at(TEXT);
",
        touched("NEW.updated_at = now();")
    ));

    let pair = migration(&old, &new, &[]).expect("the overload stages");
    assert!(
        pair.up
            .starts_with("CREATE FUNCTION touch_updated_at(stamp TEXT)"),
        "{}",
        pair.up
    );
}
