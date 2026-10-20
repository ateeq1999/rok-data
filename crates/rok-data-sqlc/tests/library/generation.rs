//! What a generation run writes, refuses, and reports.

use super::*;

#[test]
fn generate_writes_the_same_files_as_the_cli() -> Result<()> {
    let inputs = Inputs::new()?;

    rok_data_sqlc::cli::run_from([
        "rok-data-sqlc",
        "generate",
        "--schema",
        inputs.path("schema.sql").to_str().unwrap(),
        "--queries",
        inputs.path("queries.sql").to_str().unwrap(),
        "--output",
        inputs.path("cli").to_str().unwrap(),
        "--lenient",
    ])?;
    rok_data_sqlc::generate(&inputs.config("lib"))?;

    let cli = tree(&inputs.path("cli"))?;
    let lib = tree(&inputs.path("lib"))?;
    assert!(cli.contains_key(Path::new("users/models.rs")));
    assert!(cli.contains_key(Path::new("queries.rs")));
    assert_eq!(cli, lib);
    Ok(())
}

#[test]
fn report_lists_written_then_unchanged_files() -> Result<()> {
    let inputs = Inputs::new()?;
    let config = inputs.config("out");

    let first = rok_data_sqlc::generate(&config)?;
    assert!(first.written.contains(&inputs.path("out/users/models.rs")));
    assert!(first.unchanged.is_empty());

    let second = rok_data_sqlc::generate(&config)?;
    assert!(second.written.is_empty());
    assert!(
        second
            .unchanged
            .contains(&inputs.path("out/users/models.rs"))
    );
    Ok(())
}

#[test]
fn lenient_report_lists_skipped_tables_and_queries() -> Result<()> {
    let inputs = Inputs::new()?;

    let report = rok_data_sqlc::generate(&inputs.config("out"))?;

    let tables: Vec<_> = report
        .table_failures
        .iter()
        .map(|f| f.table.as_str())
        .collect();
    assert_eq!(tables, ["spans"]);
    let queries: Vec<_> = report
        .query_failures
        .iter()
        .map(|f| f.query.as_str())
        .collect();
    assert_eq!(queries, ["Broken"]);
    assert!(!inputs.path("out/spans").exists());
    Ok(())
}

#[test]
fn strict_failure_is_an_error_whose_report_names_the_table() -> Result<()> {
    let inputs = Inputs::new()?;
    let mut config = inputs.config("out");
    config.lenient = false;

    let error = rok_data_sqlc::generate(&config).unwrap_err();

    let tables: Vec<_> = error
        .report()
        .table_failures
        .iter()
        .map(|f| f.table.as_str())
        .collect();
    assert_eq!(tables, ["spans"]);
    assert!(error.to_string().contains("1 unsupported table"));
    assert!(
        !inputs.path("out").exists(),
        "a strict failure writes nothing"
    );
    Ok(())
}

#[test]
fn refused_files_are_an_error_whose_report_lists_them() -> Result<()> {
    let inputs = Inputs::new()?;
    let config = inputs.config("out");
    let guarded = inputs.path("out/users/models.rs");
    fs::create_dir_all(guarded.parent().unwrap())?;
    fs::write(&guarded, "// hand-written, precious\n")?;

    let error = rok_data_sqlc::generate(&config).unwrap_err();

    let refused: Vec<_> = error.report().refused.iter().map(|r| &r.path).collect();
    assert_eq!(refused, [&guarded]);
    assert!(error.report().written.is_empty(), "nothing was written");
    assert_eq!(fs::read_to_string(&guarded)?, "// hand-written, precious\n");
    assert!(!inputs.path("out/mod.rs").exists());
    Ok(())
}

#[test]
fn dry_run_reports_refusals_without_failing_or_writing() -> Result<()> {
    let inputs = Inputs::new()?;
    let mut config = inputs.config("out");
    config.dry_run = true;
    let guarded = inputs.path("out/users/models.rs");
    fs::create_dir_all(guarded.parent().unwrap())?;
    fs::write(&guarded, "// hand-written, precious\n")?;

    let report = rok_data_sqlc::generate(&config)?;

    assert_eq!(report.refused.len(), 1);
    assert!(report.written.contains(&inputs.path("out/mod.rs")));
    assert!(!inputs.path("out/mod.rs").exists());
    Ok(())
}

#[test]
fn schema_inputs_are_combined_in_order() -> Result<()> {
    let inputs = Inputs::new()?;
    fs::write(
        inputs.path("first.sql"),
        "CREATE TABLE users (id UUID PRIMARY KEY);",
    )?;
    fs::write(
        inputs.path("second.sql"),
        "CREATE TABLE posts (id UUID PRIMARY KEY, author_id UUID NOT NULL REFERENCES users (id));",
    )?;
    let mut config = Config::default();
    config.schema = vec![inputs.path("first.sql"), inputs.path("second.sql")];
    config.output = inputs.path("out");

    rok_data_sqlc::generate(&config)?;

    assert!(inputs.path("out/users/models.rs").exists());
    assert!(inputs.path("out/posts/models.rs").exists());
    Ok(())
}

#[test]
fn report_failures_carry_their_source_path_and_line() -> Result<()> {
    let inputs = Inputs::new()?;

    let report = rok_data_sqlc::generate(&inputs.config("out"))?;

    let table = &report.table_failures[0];
    assert_eq!(
        table.path.as_deref(),
        Some(inputs.path("schema.sql").as_path())
    );
    assert_eq!(table.line, Some(2));
    let query = &report.query_failures[0];
    assert_eq!(
        query.path.as_deref(),
        Some(inputs.path("queries.sql").as_path())
    );
    assert_eq!(query.line, Some(4));
    Ok(())
}
