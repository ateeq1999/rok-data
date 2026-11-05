//! `CREATE FUNCTION` and `DROP FUNCTION` keep the signatures query analysis
//! types calls with.

use super::*;

const TABLE: &str = "CREATE TABLE passes (id BIGSERIAL PRIMARY KEY);\n";

/// Schema, name, argument types and Rust return type.
type Signature = (Option<String>, String, Vec<String>, Option<String>);

fn functions(sql: &str) -> Vec<Signature> {
    parse(&format!("{TABLE}{sql}"))
        .schema
        .functions
        .into_iter()
        .map(|function| {
            (
                function.schema,
                function.sql_name,
                function.arg_types,
                function.return_type,
            )
        })
        .collect()
}

#[test]
fn a_function_keeps_its_argument_and_return_types() {
    assert_eq!(
        functions(
            "CREATE FUNCTION public.tz_today() RETURNS date LANGUAGE sql AS $$ SELECT current_date $$;\n\
             CREATE FUNCTION app.seats_left(pass BIGINT, OUT n INTEGER) LANGUAGE sql AS $$ SELECT 1 $$;\n\
             CREATE FUNCTION touch() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END; $$;"
        ),
        [
            (
                None,
                "tz_today".to_owned(),
                vec![],
                Some("chrono::NaiveDate".to_owned())
            ),
            (
                Some("app".to_owned()),
                "seats_left".to_owned(),
                vec!["BIGINT".to_owned()],
                None
            ),
            (None, "touch".to_owned(), vec![], None),
        ]
    );
}

#[test]
fn replace_and_drop_follow_the_signature() {
    assert_eq!(
        functions(
            "CREATE FUNCTION f(a INTEGER) RETURNS integer LANGUAGE sql AS $$ SELECT a $$;\n\
             CREATE FUNCTION f(a TEXT) RETURNS text LANGUAGE sql AS $$ SELECT a $$;\n\
             CREATE OR REPLACE FUNCTION f(b INTEGER) RETURNS bigint LANGUAGE sql AS $$ SELECT b $$;\n\
             CREATE FUNCTION g() RETURNS integer LANGUAGE sql AS $$ SELECT 1 $$;\n\
             DROP FUNCTION f(TEXT);\n\
             DROP FUNCTION IF EXISTS g;"
        ),
        [(
            None,
            "f".to_owned(),
            vec!["INTEGER".to_owned()],
            Some("i64".to_owned())
        )]
    );
}
