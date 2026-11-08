//! `-- rok:constraint <name> [<field>] "<message>"`: a constraint's field
//! and message, from anywhere in the schema dir, checked against the
//! schema and generated as the data layer's `constraints` table.

use super::*;

/// A schema with a check, a unique constraint, a foreign key, a unique
/// index and a trigger's `RAISE … CONSTRAINT`, plus `annotations`.
fn schema(annotations: &str) -> String {
    format!(
        "CREATE TABLE offer_drops (\n    id UUID PRIMARY KEY,\n    \
         month INT NOT NULL CHECK (month BETWEEN 1 AND 12),\n    code TEXT NOT NULL UNIQUE\n);\n\n\
         CREATE TABLE offers (\n    id UUID PRIMARY KEY,\n    \
         offer_drop_id UUID NOT NULL REFERENCES offer_drops (id)\n);\n\n\
         CREATE UNIQUE INDEX offers_one_per_drop ON offers (offer_drop_id);\n\n\
         CREATE FUNCTION offer_drops_window() RETURNS trigger AS $$\nBEGIN\n    \
         RAISE EXCEPTION 'the window is closed'\n        \
         USING ERRCODE = 'check_violation', CONSTRAINT = 'offer_drops_window_open';\nEND\n\
         $$ LANGUAGE plpgsql;\n\n{annotations}"
    )
}

const ANNOTATIONS: &str = "\
-- rok:constraint offer_drops_month_check month \"Pick a month from 1 to 12.\"
-- rok:constraint offer_drops_code_key code \"That code is \\\"taken\\\".\"
-- rok:constraint offers_offer_drop_id_fkey \"The offer drop no longer exists.\"
-- rok:constraint offers_one_per_drop offer_drop_id \"The drop already has an offer.\"
-- rok:constraint offer_drops_window_open \"The drop's window is closed.\"
";

/// Generates `schema` (in two files: the annotations may sit in either),
/// returning the run's outcome and the output directory.
fn generate(
    schema: &str,
    extra: &str,
) -> Result<(rok_data_sqlc::Result<rok_data_sqlc::Report>, TempDir)> {
    let dir = TempDir::new()?;
    fs::create_dir_all(dir.path().join("schema"))?;
    fs::write(dir.path().join("schema/0001_init.sql"), schema)?;
    fs::write(dir.path().join("schema/0002_messages.sql"), extra)?;
    let mut config = Config::default();
    config.schema = vec![dir.path().join("schema")];
    config.output = dir.path().join("out");
    Ok((rok_data_sqlc::generate(&config), dir))
}

#[test]
fn the_table_lists_every_annotated_constraint() -> Result<()> {
    let (outcome, dir) = generate(&schema(ANNOTATIONS), "")?;
    outcome?;
    let table = fs::read_to_string(dir.path().join("out/constraints.rs"))?;
    for entry in [
        "    Constraint {\n        name: \"offer_drops_code_key\",\n        table: Some(\"offer_drops\"),\n        \
         field: Some(\"code\"),\n        message: \"That code is \\\"taken\\\".\",\n        kind: ConstraintKind::Unique,\n    },\n",
        "    Constraint {\n        name: \"offer_drops_month_check\",\n        table: Some(\"offer_drops\"),\n        \
         field: Some(\"month\"),\n        message: \"Pick a month from 1 to 12.\",\n        kind: ConstraintKind::Check,\n    },\n",
        "    Constraint {\n        name: \"offer_drops_window_open\",\n        table: None,\n        \
         field: None,\n        message: \"The drop's window is closed.\",\n        kind: ConstraintKind::Raised,\n    },\n",
        "    Constraint {\n        name: \"offers_offer_drop_id_fkey\",\n        table: Some(\"offers\"),\n        \
         field: None,\n        message: \"The offer drop no longer exists.\",\n        kind: ConstraintKind::ForeignKey,\n    },\n",
        "    Constraint {\n        name: \"offers_one_per_drop\",\n        table: Some(\"offers\"),\n        \
         field: Some(\"offer_drop_id\"),\n        message: \"The drop already has an offer.\",\n        kind: ConstraintKind::Unique,\n    },\n",
    ] {
        assert!(table.contains(entry), "{entry}\nnot in\n{table}");
    }
    assert!(
        table.contains("pub fn lookup(name: &str) -> Option<&'static Constraint>"),
        "{table}"
    );
    assert!(
        table.contains("pub fn of_error(error: &rok_data::Error) -> Option<&'static Constraint>"),
        "{table}"
    );
    let module = fs::read_to_string(dir.path().join("out/mod.rs"))?;
    assert!(module.contains("pub mod constraints;\n"), "{module}");
    Ok(())
}

/// Annotations may sit in another file of the schema dir than the
/// constraint; a schema without any generates no table.
#[test]
fn annotations_may_sit_anywhere_and_none_means_no_table() -> Result<()> {
    let (outcome, dir) = generate(&schema(""), ANNOTATIONS)?;
    outcome?;
    assert!(dir.path().join("out/constraints.rs").is_file());

    let (outcome, dir) = generate(&schema(""), "")?;
    outcome?;
    assert!(!dir.path().join("out/constraints.rs").exists());
    let module = fs::read_to_string(dir.path().join("out/mod.rs"))?;
    assert!(!module.contains("constraints"), "{module}");
    Ok(())
}

/// An unknown constraint or field fails the run, naming it and where the
/// annotation is.
#[test]
fn an_unknown_constraint_or_field_fails_naming_it() -> Result<()> {
    let (outcome, _dir) = generate(
        &schema(""),
        "-- rok:constraint offer_drops_month_chek month \"Pick a month.\"\n",
    )?;
    let error = outcome
        .expect_err("an unknown constraint fails")
        .to_string();
    assert!(error.contains("0002_messages.sql:1"), "{error}");
    assert!(
        error.contains("`offer_drops_month_chek` is no constraint of the schema"),
        "{error}"
    );

    let (outcome, _dir) = generate(
        &schema(""),
        "\n-- rok:constraint offer_drops_month_check monht \"Pick a month.\"\n",
    )?;
    let error = outcome.expect_err("an unknown field fails").to_string();
    assert!(error.contains("0002_messages.sql:2"), "{error}");
    assert!(
        error.contains("`monht` is no column of `offer_drops`"),
        "{error}"
    );

    let (outcome, _dir) = generate(&schema(""), "-- rok:constraint offer_drops_code_key code\n")?;
    let error = outcome.expect_err("a missing message fails").to_string();
    assert!(
        error.contains("expected `-- rok:constraint <name> [<field>] \"<message>\"`"),
        "{error}"
    );
    Ok(())
}
