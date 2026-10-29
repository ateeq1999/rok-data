//! `check`: the generated code is up to date and, with `--db`, every
//! annotated query prepares against the database with the types the
//! generated code binds and decodes (engine ADR 0026).

use anyhow::{Context, Result, bail};

use super::Connect;
use super::generate::GenerateArgs;
use super::print::print_report;
use crate::check_db;

/// Checks the project `args` configures; `db` is `--db`'s value (empty for
/// a bare `--db`, which reads `DATABASE_URL`).
pub(super) fn check(
    args: &GenerateArgs,
    db: Option<&str>,
    connect: Option<&Connect>,
) -> Result<()> {
    let config = args.resolve()?;
    let (report, analysis) = match crate::generate::analyze(&config) {
        Ok(found) => found,
        Err(err) => {
            print_report(&config, err.report());
            return Err(err.into());
        }
    };
    for warning in &report.warnings {
        eprintln!("warning: {warning}");
    }
    println!(
        "generated code is up to date ({} files)",
        report.unchanged.len()
    );

    let Some(db) = db else {
        return Ok(());
    };
    let url = match db {
        "" => std::env::var("DATABASE_URL")
            .context("`--db` without a URL reads DATABASE_URL, which is not set")?,
        url => url.to_owned(),
    };
    let connect = connect.context(
        "`check --db` connects through the `rok` binary's runtime: run `rok sqlc check --db`",
    )?;
    if analysis.queries.is_empty() {
        println!("no annotated queries to check against the database");
        return Ok(());
    }
    let mut describer = connect(&url)?;
    let outcome = check_db::check(&analysis, describer.as_mut());
    for mismatch in &outcome.mismatches {
        println!("{mismatch}");
    }
    for unchecked in &outcome.unchecked {
        eprintln!("note: not checked, the type is not one the check knows: {unchecked}");
    }
    let count = outcome.checked;
    let noun = if count == 1 { "query" } else { "queries" };
    if !outcome.mismatches.is_empty() {
        bail!(
            "{} mismatch(es) in {count} {noun} checked against the database",
            outcome.mismatches.len()
        );
    }
    println!("{count} {noun} agree with the database");
    Ok(())
}
