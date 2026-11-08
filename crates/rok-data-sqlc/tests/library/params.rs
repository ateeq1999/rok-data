//! A query's parameter shape: positional arguments or its `…Params`
//! struct, fixed per query by `-- rok:params` and per project by
//! `[queries] params`, so adding a parameter never flips a call shape.

use super::*;

/// A two-parameter and a five-parameter query, each annotated with
/// `two` and `five` (an annotation line, or nothing).
fn queries(two: &str, five: &str) -> String {
    format!(
        "-- name: SetEmail :exec\n{two}UPDATE users SET email = $2 WHERE id = $1;\n\n\
         -- name: FindAny :many\n{five}SELECT id FROM users \
         WHERE email = $1 OR email = $2 OR email = $3 OR email = $4 OR email = $5;\n"
    )
}

/// Generates `queries` with `settings` as the config file's extra lines,
/// and returns the generated `queries.rs`.
fn generated(queries: &str, settings: &str) -> Result<String> {
    let dir = TempDir::new()?;
    fs::write(
        dir.path().join("schema.sql"),
        "CREATE TABLE users (id UUID PRIMARY KEY, email TEXT NOT NULL);\n",
    )?;
    fs::write(dir.path().join("queries.sql"), queries)?;
    let config = dir.path().join("rok-data-sqlc.toml");
    fs::write(
        &config,
        format!("schema = \"schema.sql\"\noutput = \"out\"\n{settings}"),
    )?;
    rok_data_sqlc::cli::run_from([
        "rok-data-sqlc",
        "generate",
        "--config",
        config.to_str().unwrap(),
    ])?;
    Ok(fs::read_to_string(dir.path().join("out/queries.rs"))?)
}

/// The paths section of the config file: the query file alone, or with
/// `params`.
fn section(params: Option<&str>) -> String {
    match params {
        None => "queries = \"queries.sql\"\n".to_owned(),
        Some(params) => {
            format!("[queries]\npaths = \"queries.sql\"\nparams = \"{params}\"\n")
        }
    }
}

/// Today's rule, unannotated: positional up to three, a struct from four,
/// and the doc comment as it was.
#[test]
fn auto_keeps_todays_shapes() -> Result<()> {
    let code = generated(&queries("", ""), &section(None))?;
    assert!(code.contains("id: UserId, email: &str"), "{code}");
    assert!(code.contains("pub struct FindAnyParams {"), "{code}");
    assert!(!code.contains("fixes its shape"), "{code}");
    assert_eq!(code, generated(&queries("", ""), &section(Some("auto")))?);
    Ok(())
}

/// `-- rok:params struct` on two parameters makes the struct;
/// `positional` on five keeps the arguments; each doc says why.
#[test]
fn the_annotation_fixes_the_shape() -> Result<()> {
    let code = generated(
        &queries("-- rok:params struct\n", "-- rok:params positional\n"),
        &section(None),
    )?;
    assert!(code.contains("pub struct SetEmailParams {"), "{code}");
    assert!(code.contains("params: SetEmailParams"), "{code}");
    assert!(!code.contains("pub struct FindAnyParams"), "{code}");
    assert!(code.contains("email_5: &str"), "{code}");
    assert!(
        code.contains("/// Takes its parameters as `SetEmailParams`: `-- rok:params struct`."),
        "{code}"
    );
    assert!(
        code.contains("/// Takes its parameters positionally: `-- rok:params positional`."),
        "{code}"
    );
    Ok(())
}

/// `[queries] params = "struct"` gives every query its struct; a query's
/// annotation still wins.
#[test]
fn the_project_sets_the_default_and_the_annotation_wins() -> Result<()> {
    let code = generated(&queries("", ""), &section(Some("struct")))?;
    assert!(code.contains("params: SetEmailParams"), "{code}");
    assert!(code.contains("params: FindAnyParams"), "{code}");
    assert!(
        code.contains(
            "/// Takes its parameters as `SetEmailParams`: `[queries] params = \"struct\"`."
        ),
        "{code}"
    );

    let code = generated(
        &queries("-- rok:params positional\n", ""),
        &section(Some("struct")),
    )?;
    assert!(!code.contains("pub struct SetEmailParams"), "{code}");
    assert!(code.contains("params: FindAnyParams"), "{code}");

    let code = generated(&queries("", ""), &section(Some("positional")))?;
    assert!(!code.contains("pub struct FindAnyParams"), "{code}");
    Ok(())
}

/// A misspelled shape fails, naming the ones there are.
#[test]
fn an_unknown_shape_fails() -> Result<()> {
    let inputs = Inputs::new()?;
    fs::write(
        inputs.path("schema.sql"),
        "CREATE TABLE users (id UUID PRIMARY KEY, email TEXT NOT NULL);\n",
    )?;
    fs::write(
        inputs.path("queries.sql"),
        queries("-- rok:params structs\n", ""),
    )?;
    let mut config = inputs.config("out");
    config.lenient = false;
    let error = rok_data_sqlc::generate(&config).expect_err("an unknown annotation fails");
    let reasons: Vec<&str> = error
        .report()
        .query_failures
        .iter()
        .map(|failure| failure.reason.as_str())
        .collect();
    assert_eq!(
        reasons,
        [
            "line 2: unknown parameter shape `structs` in `-- rok:params`: \
          expected `struct` or `positional`"
        ]
    );

    let error = generated(&queries("", ""), &section(Some("structs")))
        .expect_err("an unknown setting fails");
    assert!(format!("{error:#}").contains("structs"), "{error:#}");
    Ok(())
}
