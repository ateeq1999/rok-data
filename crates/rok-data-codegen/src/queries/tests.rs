//! The query generator's own tests.

use super::*;

fn q(sql: &str) -> Query {
    Query {
        name: "q".into(),
        module: "m".into(),
        kind: QueryKind::Many,
        sql: sql.into(),
        doc: vec![],
        param_names: vec![],
        location: "db/m.sql:1".into(),
    }
}

#[test]
fn parameter_names() {
    let s = shape(&q("SELECT * FROM users u WHERE u.email = $1 AND $2 < age AND created_at BETWEEN $3 AND $4 AND id = ANY($5) AND role IN ($6) LIMIT $7 OFFSET $8")).unwrap();
    assert_eq!(
        s.param_names,
        [
            "email",
            "age",
            "created_at_from",
            "created_at_to",
            "ids",
            "roles",
            "limit",
            "offset"
        ]
    );
    assert_eq!(s.reads, ["users"]);
    assert!(s.writes.is_empty());

    let s = shape(&q(
        "INSERT INTO app.posts (author_id, title, views) VALUES ($1, lower($2), $3) RETURNING *",
    ))
    .unwrap();
    assert_eq!(s.param_names, ["author_id", "p2", "views"]);
    assert_eq!(s.writes, ["app.posts"]);

    let s = shape(&q(
        "UPDATE users SET name = $2 WHERE id = $1 AND name <> $2",
    ))
    .unwrap();
    assert_eq!(s.param_names, ["id", "name"]);
    assert_eq!(s.writes, ["users"]);

    let s = shape(&q(
        "SELECT 1 FROM users WHERE lower(email) = lower($1) AND trim($2) = name",
    ))
    .unwrap();
    assert_eq!(s.param_names, ["email", "name"]);

    let s = shape(&q("DELETE FROM posts WHERE id = $1 OR id = $2")).unwrap();
    assert_eq!(s.param_names, ["id", "id_2"]);
    assert_eq!(s.writes, ["posts"]);
}

#[test]
fn param_overrides() {
    let mut query = q("SELECT * FROM t WHERE x > now() - $1::interval");
    query.param_names = vec![(1, "age".into())];
    assert_eq!(shape(&query).unwrap().param_names, ["age"]);
    query.param_names = vec![(2, "nope".into())];
    assert!(shape(&query).is_err());
}
