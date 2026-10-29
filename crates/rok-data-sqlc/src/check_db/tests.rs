use super::*;
use crate::queries::{AnalyzedQuery, OutputColumn, Param, ReturnMode};

/// Answers each SQL with a canned statement, or fails to prepare it.
struct Canned(Vec<(&'static str, Result<Described, String>)>);

impl Describer for Canned {
    fn describe(&mut self, sql: &str) -> Result<Described, String> {
        self.0
            .iter()
            .find(|(canned, _)| *canned == sql)
            .map_or_else(
                || Err(format!("no canned answer for {sql}")),
                |(_, answer)| answer.clone(),
            )
    }
}

fn query(name: &str, sql: &str, params: &[(&str, &str)], result: ResultShape) -> LocatedQuery {
    LocatedQuery {
        path: PathBuf::from("queries/media.sql"),
        line: 7,
        query: AnalyzedQuery {
            name: name.to_owned(),
            fn_name: name.to_lowercase(),
            mode: ReturnMode::Many,
            sql: sql.to_owned(),
            params: params
                .iter()
                .enumerate()
                .map(|(index, (name, rust_type))| Param {
                    index: index + 1,
                    name: (*name).to_owned(),
                    rust_type: (*rust_type).to_owned(),
                })
                .collect(),
            result,
        },
    }
}

fn row(columns: &[(&str, &str)]) -> ResultShape {
    ResultShape::Row(
        columns
            .iter()
            .map(|(name, rust_type)| OutputColumn {
                sql_name: (*name).to_owned(),
                rust_name: (*name).to_owned(),
                rust_type: (*rust_type).to_owned(),
            })
            .collect(),
    )
}

fn column(name: &str, type_name: &str, nullable: Option<bool>) -> DescribedColumn {
    DescribedColumn {
        name: name.to_owned(),
        type_name: type_name.to_owned(),
        nullable,
    }
}

fn run(queries: Vec<LocatedQuery>, canned: Canned) -> Outcome {
    let analysis = Analysis {
        queries,
        ..Analysis::default()
    };
    check(&analysis, &mut { canned })
}

const SQL: &str = "SELECT id, filename FROM media WHERE owner_id = $1";

#[test]
fn a_query_the_database_agrees_with_has_no_mismatch() {
    let outcome = run(
        vec![query(
            "Owned",
            SQL,
            &[("owner_id", "i64")],
            row(&[("id", "uuid::Uuid"), ("filename", "Option<String>")]),
        )],
        Canned(vec![(
            SQL,
            Ok(Described {
                params: vec!["INT8".into()],
                columns: vec![
                    column("id", "UUID", Some(false)),
                    column("filename", "TEXT", Some(true)),
                ],
            }),
        )]),
    );
    assert_eq!(outcome.checked, 1);
    assert_eq!(outcome.mismatches, []);
    assert!(outcome.unchecked.is_empty());
}

#[test]
fn types_names_counts_and_nullability_are_compared() {
    let outcome = run(
        vec![query(
            "Owned",
            SQL,
            &[("owner_id", "i32")],
            row(&[("id", "uuid::Uuid"), ("name", "String")]),
        )],
        Canned(vec![(
            SQL,
            Ok(Described {
                params: vec!["INT8".into()],
                columns: vec![
                    column("id", "UUID", None),
                    column("filename", "TEXT", Some(true)),
                    column("extra", "INT4", None),
                ],
            }),
        )]),
    );
    let messages: Vec<String> = outcome.mismatches.iter().map(ToString::to_string).collect();
    assert_eq!(
        messages,
        [
            "queries/media.sql:7: Owned: parameter `$1` (`owner_id`) is `INT8` in the database; the generated code uses `i32`",
            "queries/media.sql:7: Owned: column 2 is `filename` in the database; the generated code reads `name`",
            "queries/media.sql:7: Owned: column `filename` can be NULL in the database, but decodes into `String`",
            "queries/media.sql:7: Owned: answers 3 column(s) in the database; the generated code reads 2",
        ]
    );
}

#[test]
fn a_query_that_does_not_prepare_is_a_mismatch_and_unknown_types_are_unchecked() {
    let outcome = run(
        vec![
            query(
                "Broken",
                "SELECT nope",
                &[],
                ResultShape::Scalar("i64".into()),
            ),
            query(
                "Custom",
                "SELECT $1::text",
                &[("value", "my::Thing")],
                ResultShape::Scalar("Option<String>".into()),
            ),
        ],
        Canned(vec![
            ("SELECT nope", Err("column \"nope\" does not exist".into())),
            (
                "SELECT $1::text",
                Ok(Described {
                    params: vec!["TEXT".into()],
                    columns: vec![column("text", "TEXT", None)],
                }),
            ),
        ]),
    );
    let messages: Vec<String> = outcome.mismatches.iter().map(ToString::to_string).collect();
    assert_eq!(
        messages,
        ["queries/media.sql:7: Broken: does not prepare: column \"nope\" does not exist"]
    );
    assert_eq!(
        outcome.unchecked,
        ["Custom: parameter `$1` (`value`) (`my::Thing`)"]
    );
}

#[test]
fn an_exec_query_reads_no_columns() {
    let sql = "DELETE FROM media WHERE id = $1";
    let outcome = run(
        vec![query(
            "Remove",
            sql,
            &[("id", "uuid::Uuid")],
            ResultShape::None,
        )],
        Canned(vec![(
            sql,
            Ok(Described {
                params: vec!["UUID".into()],
                columns: Vec::new(),
            }),
        )]),
    );
    assert_eq!(outcome.mismatches, []);
}
