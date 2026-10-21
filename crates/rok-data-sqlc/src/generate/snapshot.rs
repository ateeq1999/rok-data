//! The run-level undo strip and the SQL snapshot it feeds.
//!
//! The undos a truth carries are the reverse half of migration pairs, not
//! schema (engine ADR 0017): executed, an undo SQL would undo the change
//! the truth means to have. The statement path of the parser
//! (`parser/postgres/statements.rs`) strips them per source for its own
//! reading; this module owns the run-level gesture on top of that —
//! [`prepared`] strips every source once, and [`sql`] composes the
//! snapshot the generated `schema.rs` carries out of the stripped text.

use std::path::PathBuf;

use crate::parser;

/// The engine's schema inputs with the `-- rok:down` undos stripped
/// (`parser/postgres/undo.rs`'s strip, which refuses a misplaced marker),
/// in their given order. The parse below and the snapshot SQL the
/// generated `schema.rs` carries both read this run's output; the parser
/// strips each source again for its own reading, and finds nothing to
/// skip when it does.
pub(crate) fn prepared(sources: Vec<(PathBuf, String)>) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let sources = sources
        .into_iter()
        .map(|(path, sql)| {
            // The strip is the audience of the undo refusals of a truth
            // file: the first thing a run does with a misplaced marker,
            // before anything parses.
            let sql = parser::postgres::undo::strip_undos(&sql, &path)?;
            anyhow::Ok((path, sql.into_owned()))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(sources)
}

/// The schema snapshot: every input verbatim, in order, each starting on a
/// new line.
pub(crate) fn sql(sources: &[(PathBuf, String)]) -> String {
    let mut sql = String::new();
    for (_, text) in sources {
        if !sql.is_empty() && !sql.ends_with('\n') {
            sql.push('\n');
        }
        sql.push_str(text);
    }
    sql
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The snapshot SQL is the run-level strip's audience: an `ALTER` and
    /// the undo statement its `-- rok:down` marker introduces leave the
    /// snapshot with the change the truth means to have and no
    /// `DROP COLUMN` behind. The full pipeline's pin is
    /// `the_snapshot_leaves_out_the_truths_undo_statements`
    /// (`tests/library/generation.rs`).
    #[test]
    fn the_snapshot_sql_carries_no_undos() {
        let sources = prepared(vec![(
            PathBuf::from("books.sql"),
            "CREATE TABLE books (id BIGSERIAL PRIMARY KEY);\n\
             ALTER TABLE books ADD COLUMN isbn TEXT;\n\
             -- rok:down\n\
             ALTER TABLE books DROP COLUMN isbn;\n"
                .to_owned(),
        )])
        .expect("the source parses");
        let sql = sql(&sources);
        assert!(sql.contains("ADD COLUMN isbn"), "{sql}");
        assert!(!sql.contains("DROP COLUMN isbn"), "{sql}");
        assert!(!sql.contains("rok:down"), "{sql}");
    }
}
