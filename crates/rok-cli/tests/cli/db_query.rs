//! `rok db query`: every statement in one transaction that commits only on
//! success (ADR 0028), driven through the `rok` binary.

use crate::db_support::{
    Sandbox, TestDb, assert_error, assert_success, database_url_or_skip, json, stdout,
};

/// `rok db query SQL --database-url URL [extra…]`.
fn query(url: &str, sql: &str, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["db", "query", sql, "--database-url", url];
    args.extend_from_slice(extra);
    Sandbox::new().run(&args)
}

/// The single value a one-row, one-column `SELECT` returns.
fn scalar(url: &str, sql: &str) -> serde_json::Value {
    let output = query(url, sql, &["--json"]);
    assert_success(&output);
    json(&output)["results"][0]["rows"][0][0].clone()
}

#[test]
fn db_query_with_no_statement_is_an_error() {
    // Fails before connecting: the URL points nowhere.
    let output = query("postgres://u:p@127.0.0.1:1/none", " ; -- nothing\n", &[]);
    assert_error(&output, "no SQL statement to run");
}

#[test]
fn db_query_refuses_transaction_control() {
    // Fails before connecting: COMMIT would end the transaction early.
    let output = query(
        "postgres://u:p@127.0.0.1:1/none",
        "INSERT INTO t VALUES (1); COMMIT; DROP TABLE t",
        &[],
    );
    assert_error(&output, "statement 2 is a `COMMIT`");
}

#[test]
fn db_query_commit() {
    let Some(base) = database_url_or_skip("db_query_commit") else {
        return;
    };
    let db = TestDb::create(&base, "commit");

    let output = query(
        &db.url,
        "CREATE TABLE t (id serial PRIMARY KEY); INSERT INTO t DEFAULT VALUES;",
        &[],
    );
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "OK: 2 statements executed, transaction committed.\n"
    );
    assert_eq!(scalar(&db.url, "SELECT count(*) FROM t"), "1");
}

#[test]
fn db_query_rollback() {
    let Some(base) = database_url_or_skip("db_query_rollback") else {
        return;
    };
    let db = TestDb::create(&base, "rollback");

    let output = query(
        &db.url,
        "CREATE TABLE t (id int); INSERT INTO t VALUES (1); SELECT 1/0;",
        &[],
    );
    assert_error(&output, "statement 3 failed: division by zero");
    assert!(crate::db_support::stderr(&output).contains("Transaction rolled back."));
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));

    // The table the first statement created went with the rollback.
    assert_eq!(
        scalar(
            &db.url,
            "SELECT count(*) FROM pg_tables WHERE tablename = 't'"
        ),
        "0"
    );
}

#[test]
fn db_query_transaction_isolation() {
    let Some(base) = database_url_or_skip("db_query_transaction_isolation") else {
        return;
    };
    let db = TestDb::create(&base, "isolation");
    assert_success(&query(
        &db.url,
        "CREATE TABLE t (id int PRIMARY KEY); INSERT INTO t VALUES (1)",
        &[],
    ));

    // The second insert violates the key; the first must not survive either.
    let output = query(
        &db.url,
        "INSERT INTO t VALUES (2); INSERT INTO t VALUES (1)",
        &[],
    );
    assert_error(&output, "statement 2 failed: duplicate key value");

    // The state committed earlier is untouched, and nothing new is in.
    assert_eq!(
        scalar(&db.url, "SELECT string_agg(id::text, ',') FROM t"),
        "1"
    );
}

#[test]
fn db_query_select_tabular() {
    let Some(base) = database_url_or_skip("db_query_select_tabular") else {
        return;
    };
    let db = TestDb::create(&base, "select_text");

    let output = query(
        &db.url,
        "SELECT 1 AS id, 'alice' AS name UNION ALL SELECT 22, NULL; SELECT 'x' AS only WHERE false",
        &[],
    );
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "id | name\n\
         ---+------\n\
         1  | alice\n\
         22 |\n\
         (2 rows)\n\
         only\n\
         ----\n\
         (0 rows)\n\
         OK: 2 statements executed, transaction committed.\n"
    );
}

#[test]
fn db_query_select_json() {
    let Some(base) = database_url_or_skip("db_query_select_json") else {
        return;
    };
    let db = TestDb::create(&base, "select_json");

    let output = query(
        &db.url,
        "CREATE TABLE t (id int, note text); \
         INSERT INTO t VALUES (1, 'a'), (2, NULL); \
         SELECT id, note FROM t ORDER BY id; \
         SELECT now() AS at WHERE false",
        &["--json"],
    );
    assert_success(&output);
    let report = json(&output);
    assert_eq!(report["statements"], 4);
    assert_eq!(report["committed"], true);
    let results = report["results"].as_array().expect("results");
    assert_eq!(results.len(), 4);
    assert_eq!(results[1]["rows_affected"], 2);
    assert_eq!(
        results[2]["columns"],
        serde_json::json!([{ "name": "id", "type": "INT4" }, { "name": "note", "type": "TEXT" }])
    );
    assert_eq!(
        results[2]["rows"],
        serde_json::json!([["1", "a"], ["2", null]])
    );
    assert_eq!(results[2]["row_count"], 2);
    // A SELECT with no rows still reports its columns.
    assert_eq!(results[3]["columns"][0]["name"], "at");
    assert_eq!(results[3]["row_count"], 0);
}

#[test]
fn db_query_json_error() {
    let Some(base) = database_url_or_skip("db_query_json_error") else {
        return;
    };
    let db = TestDb::create(&base, "json_error");

    let output = query(&db.url, "SELECT 1; SELECT 1/0", &["--json"]);
    assert_eq!(output.status.code(), Some(1));
    let report = json(&output);
    assert_eq!(
        report,
        serde_json::json!({
            "statements": 2,
            "committed": false,
            "error": { "statement": 2, "message": "division by zero" },
        })
    );
}

#[test]
fn db_query_splits_around_quotes_and_comments() {
    let Some(base) = database_url_or_skip("db_query_splits_around_quotes_and_comments") else {
        return;
    };
    let db = TestDb::create(&base, "split");

    let sql = "CREATE TABLE t (note text); -- a comment; not a statement\n\
         CREATE FUNCTION add_note(n text) RETURNS void AS $body$ \
           BEGIN INSERT INTO t VALUES (n); INSERT INTO t VALUES (n || ';2'); END; \
         $body$ LANGUAGE plpgsql; \
         /* a block; comment */ SELECT add_note('a;b'); \
         INSERT INTO t VALUES ($$c;d$$), (E'e\\';f');";
    let output = query(&db.url, sql, &["--json"]);
    assert_success(&output);
    assert_eq!(json(&output)["statements"], 4);
    assert_eq!(
        scalar(
            &db.url,
            "SELECT string_agg(note, '|' ORDER BY note COLLATE \"C\") FROM t"
        ),
        "a;b|a;b;2|c;d|e';f"
    );
}

#[test]
fn db_query_on_a_server() {
    let Some(base) = database_url_or_skip("db_query_on_a_server") else {
        return;
    };
    let db = TestDb::create(&base, "server");
    let sandbox = Sandbox::new();
    sandbox.add_server("qa-box", &db.url);

    let output = sandbox
        .rok(&[
            "db",
            "query",
            "SELECT current_database()",
            "--server",
            "qa-box",
            "--json",
        ])
        .env(
            "ROK_SERVER_QA_BOX_PASSWORD",
            base.password().unwrap_or_default(),
        )
        .output()
        .expect("spawn rok");
    assert_success(&output);
    assert_eq!(json(&output)["results"][0]["rows"][0][0], db.name.as_str());
}
