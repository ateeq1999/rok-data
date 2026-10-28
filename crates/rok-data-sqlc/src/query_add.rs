//! `rok sqlc query add`: one annotated query appended to a query file and
//! generated in the same step. The file only keeps the query when the
//! generation succeeds; otherwise it is restored and the error says why.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::config_file;
use crate::queries::ReturnMode;
use crate::report::display_path;

/// What `query add` was asked for.
#[derive(Debug)]
pub(crate) struct QueryAdd {
    /// The query's name, in `PascalCase`: `OwnedMedia`.
    pub(crate) name: String,
    /// The SQL, with `$n` placeholders.
    pub(crate) sql: String,
    /// The query file: a path, or a stem (`media`) under the configured
    /// query directory.
    pub(crate) file: Option<String>,
    /// The return mode, or `None` to infer it from the SQL ([`infer_mode`]).
    pub(crate) mode: Option<ReturnMode>,
    /// The config file, or `None` to discover `rok-data-sqlc.toml`.
    pub(crate) config: Option<PathBuf>,
}

/// Appends the query to its file and generates; restores the file and fails
/// when the generation does.
pub(crate) fn run(add: QueryAdd) -> Result<(PathBuf, crate::Report)> {
    check_name(&add.name)?;
    let config_path = match add.config {
        Some(path) => path,
        None => config_file::discover(&std::env::current_dir()?).with_context(|| {
            format!(
                "no {} found in the current directory or its parents",
                config_file::FILE_NAME
            )
        })?,
    };
    let config = config_file::load_any(&config_path)?.into_config();
    let file = query_file(&config.queries, add.file.as_deref())?;
    let before = match fs::read_to_string(&file) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", display_path(&file)));
        }
    };
    if let Some(text) = &before
        && declares(text, &add.name)
    {
        bail!(
            "{} already has a query named `{}`",
            display_path(&file),
            add.name
        );
    }
    let mode = add.mode.unwrap_or_else(|| infer_mode(&add.sql));
    let mut text = before.clone().unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&annotated(&add.name, mode, &add.sql));
    fs::write(&file, &text).with_context(|| format!("failed to write {}", display_path(&file)))?;

    match crate::generate(&config) {
        Ok(report) => Ok((file, report)),
        Err(err) => {
            match &before {
                Some(text) => fs::write(&file, text),
                None => fs::remove_file(&file),
            }
            .with_context(|| format!("failed to restore {}", display_path(&file)))?;
            // The failures say why, with their file and line.
            let report = err.report();
            let reasons: Vec<String> = report
                .query_failures
                .iter()
                .map(ToString::to_string)
                .chain(report.table_failures.iter().map(ToString::to_string))
                .collect();
            let mut message = format!("`{}` was not added to {}", add.name, display_path(&file));
            for reason in reasons {
                message.push_str(&format!("\n  {reason}"));
            }
            Err(anyhow::Error::from(err).context(message))
        }
    }
}

/// The query file `file` names: a path (`queries/media.sql`), or a stem
/// (`media`) under the configured `queries` directory. With no `file`, the
/// configured `queries` must name exactly one file.
fn query_file(configured: &[PathBuf], file: Option<&str>) -> Result<PathBuf> {
    let dirs: Vec<&PathBuf> = configured.iter().filter(|path| path.is_dir()).collect();
    match file {
        Some(file) if file.ends_with(".sql") => Ok(PathBuf::from(file)),
        Some(stem) => match dirs.as_slice() {
            [dir] => Ok(dir.join(format!("{stem}.sql"))),
            [] => bail!("no `queries` directory is configured to put `{stem}.sql` in; pass a path"),
            _ => bail!("several `queries` directories are configured; pass a path"),
        },
        None => match configured {
            [only] if !only.is_dir() => Ok(only.clone()),
            _ => bail!("pass --file: the query file (`media`, or a path ending in `.sql`)"),
        },
    }
}

/// Whether a query name is `PascalCase`: an ASCII capital, then letters and
/// digits.
fn check_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_alphanumeric());
    if !valid {
        bail!("query name `{name}` is not PascalCase (`OwnedMedia`)");
    }
    Ok(())
}

/// Whether `text` already annotates a query named `name`.
fn declares(text: &str, name: &str) -> bool {
    text.lines().any(|line| {
        line.trim_start()
            .strip_prefix("-- name:")
            .and_then(|rest| rest.split_whitespace().next())
            == Some(name)
    })
}

/// The query as the file holds it: its annotation, then the SQL ending in
/// `;`.
fn annotated(name: &str, mode: ReturnMode, sql: &str) -> String {
    let sql = sql.trim().trim_end_matches(';').trim_end();
    format!("-- name: {name} :{}\n{sql};\n", mode.keyword())
}

/// The return mode the SQL most likely means; `--one`, `--maybe-one`,
/// `--many`, `--exec` and `--execrows` override it.
///
/// - an `INSERT … RETURNING` returns the inserted row: `:one`;
/// - any other statement returning rows (a `SELECT`, a `RETURNING`) is
///   `:many`, or `:maybe_one` when it ends in `LIMIT 1`;
/// - a statement returning nothing reports its affected rows: `:execrows`.
pub(crate) fn infer_mode(sql: &str) -> ReturnMode {
    let words: Vec<String> = sql
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_uppercase)
        .collect();
    let first = words.first().map(String::as_str);
    let returning = words.iter().any(|word| word == "RETURNING");
    let limit_one = words
        .windows(2)
        .last()
        .is_some_and(|pair| pair[0] == "LIMIT" && pair[1] == "1");
    match first {
        Some("INSERT") if returning => ReturnMode::One,
        Some("INSERT" | "UPDATE" | "DELETE") if !returning => ReturnMode::ExecRows,
        _ if limit_one => ReturnMode::MaybeOne,
        _ => ReturnMode::Many,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_the_mode_from_the_statement() {
        let mode = |sql| infer_mode(sql).keyword();
        assert_eq!(mode("SELECT * FROM media WHERE owner_id = $1"), "many");
        assert_eq!(
            mode("select * from media where id = $1 limit 1"),
            "maybe_one"
        );
        assert_eq!(
            mode("INSERT INTO media (id) VALUES ($1) RETURNING *"),
            "one"
        );
        assert_eq!(mode("UPDATE media SET size = $2 WHERE id = $1"), "execrows");
        assert_eq!(
            mode("DELETE FROM media WHERE id = $1 RETURNING storage_key"),
            "many"
        );
        assert_eq!(
            mode("WITH gone AS (DELETE FROM media RETURNING id) SELECT count(*) FROM gone"),
            "many"
        );
    }

    #[test]
    fn annotates_and_terminates_the_sql() {
        assert_eq!(
            annotated("OwnedMedia", ReturnMode::MaybeOne, "  SELECT 1;  "),
            "-- name: OwnedMedia :maybe_one\nSELECT 1;\n"
        );
    }

    #[test]
    fn names_are_pascal_case_and_unique() {
        assert!(check_name("OwnedMedia").is_ok());
        assert!(check_name("owned_media").is_err());
        assert!(check_name("Owned-Media").is_err());
        let text = "-- name: OwnedMedia :one\nSELECT 1;\n";
        assert!(declares(text, "OwnedMedia"));
        assert!(!declares(text, "Owned"));
    }

    #[test]
    fn a_stem_resolves_under_the_one_query_directory() {
        let dir = tempfile::tempdir().unwrap();
        let queries = vec![dir.path().to_path_buf()];
        assert_eq!(
            query_file(&queries, Some("media")).unwrap(),
            dir.path().join("media.sql")
        );
        assert_eq!(
            query_file(&queries, Some("other/x.sql")).unwrap(),
            PathBuf::from("other/x.sql")
        );
        assert!(query_file(&queries, None).is_err());
        assert!(query_file(&[], Some("media")).is_err());
    }
}
