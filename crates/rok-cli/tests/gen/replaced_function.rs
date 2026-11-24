//! `rok gen --migration` replaces a changed function in place: its
//! triggers stay, so the migration applies on a database that has them.

use super::*;

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
CREATE TABLE {table} (
    id BIGINT PRIMARY KEY,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER {table}_touch BEFORE UPDATE ON {table}
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
-- rok:down
DROP TRIGGER {table}_touch ON {table};
"
        ));
    }
    text
}

/// A project in the divided layout: the truth in `schema/`, the staged
/// pairs in `history/`.
fn divided(schema: &str) -> Result<TempDir> {
    let dir = TempDir::new()?;
    fs::create_dir_all(dir.path().join("schema"))?;
    fs::write(dir.path().join("schema/app.sql"), schema)?;
    Ok(dir)
}

fn stage(dir: &Path, name: &str) -> Result<Output> {
    run_in(
        dir,
        &[
            "gen",
            "--schema",
            "schema",
            "--migrations-dir",
            "history",
            "--migration",
            name,
        ],
    )
}

#[test]
fn a_changed_trigger_function_is_replaced_and_applies() -> Result<()> {
    let dir = divided(&touched("NEW.updated_at = now();"))?;
    assert_success(&stage(dir.path(), "init")?);
    fs::write(
        dir.path().join("schema/app.sql"),
        touched("NEW.updated_at = clock_timestamp();"),
    )?;
    assert_success(&stage(dir.path(), "touch_clock")?);

    let up = fs::read_to_string(dir.path().join("history/0002_touch_clock.up.sql"))?;
    assert_eq!(up.matches("CREATE OR REPLACE FUNCTION").count(), 1, "{up}");
    assert!(!up.contains("DROP"), "{up}");
    assert!(!up.contains("TRIGGER"), "{up}");

    // On a database where three triggers use it, the pair applies.
    let Some(base) = std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty()) else {
        eprintln!(
            "skipping a_changed_trigger_function_is_replaced_and_applies' database half: DATABASE_URL not set"
        );
        return Ok(());
    };
    let name = format!("rok_t_replaced_fn_{}", std::process::id());
    let mut admin = url::Url::parse(&base)?;
    admin.set_path("/postgres");
    let mut url = admin.clone();
    url.set_path(&format!("/{name}"));

    assert_success(&run_in(
        dir.path(),
        &["db", "create", &name, "--database-url", admin.as_str()],
    )?);
    let applied = run_in(
        dir.path(),
        &[
            "migrate",
            "up",
            "--dir",
            "history",
            "--database-url",
            url.as_str(),
        ],
    );
    let undone = run_in(
        dir.path(),
        &[
            "migrate",
            "down",
            "--dir",
            "history",
            "--database-url",
            url.as_str(),
        ],
    );
    let dropped = run_in(
        dir.path(),
        &[
            "db",
            "drop",
            &name,
            "--database-url",
            admin.as_str(),
            "--yes",
            "--force",
        ],
    )?;
    let applied = applied?;
    assert_success(&applied);
    assert!(
        stdout(&applied).contains("applied 2 touch_clock"),
        "{}",
        stdout(&applied)
    );
    assert_success(&undone?);
    assert_success(&dropped);
    Ok(())
}

#[test]
fn a_changed_signature_is_refused_naming_the_triggers() -> Result<()> {
    let dir = divided(&touched("NEW.updated_at = now();"))?;
    assert_success(&stage(dir.path(), "init")?);
    fs::write(
        dir.path().join("schema/app.sql"),
        touched("NEW.updated_at = now();")
            .replace(
                "touch_updated_at() RETURNS",
                "touch_updated_at(stamp TEXT) RETURNS",
            )
            .replace(
                "DROP FUNCTION touch_updated_at()",
                "DROP FUNCTION touch_updated_at(TEXT)",
            ),
    )?;

    let refused = stage(dir.path(), "stamp")?;
    assert_eq!(refused.status.code(), Some(1));
    let stderr = stderr(&refused);
    assert!(stderr.contains("touch_updated_at(text)"), "{stderr}");
    for trigger in ["users_touch", "posts_touch", "tags_touch"] {
        assert!(stderr.contains(trigger), "{stderr}");
    }
    assert!(!dir.path().join("history/0002_stamp.up.sql").exists());
    Ok(())
}
