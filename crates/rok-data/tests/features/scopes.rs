//! Named scopes: the model's entry, the chain's method, and composition
//! (spec D7).
//!
//! The SQL-shape tests always run; the database test needs `DATABASE_URL`
//! (see `tests/postgres.rs`).

use super::*;

/// A model with two named scopes.
#[derive(Debug, Clone, PartialEq, Model)]
#[rok(scopes(popular = popular, active = active, older_than(lim: i32) = older_than))]
struct Visitor {
    #[rok(generated)]
    id: i64,
    name: String,
    age: i32,
    views: i32,
}

/// The `popular` scope: the rows with many views.
fn popular(query: Select<Visitor>) -> Select<Visitor> {
    query.filter(Visitor::VIEWS.gt(100))
}

/// The `active` scope: the rows of an adult.
fn active(query: Select<Visitor>) -> Select<Visitor> {
    query.filter(Visitor::AGE.gte(18))
}

/// The `older_than` scope: the rows older than a given age. A
/// parameterized scope: the free function takes the query, then the
/// declared parameters.
fn older_than(query: Select<Visitor>, lim: i32) -> Select<Visitor> {
    query.filter(Visitor::AGE.gt(lim))
}

/// A model with a default scope and a named scope.
#[derive(Debug, Clone, PartialEq, Model)]
#[rok(default_scope = published_only, scopes(trending = trending))]
struct Note {
    #[rok(generated)]
    id: i64,
    title: String,
    published: bool,
    views: i32,
}

/// The model's default scope: the published rows.
fn published_only() -> Expr<Note> {
    Note::PUBLISHED.eq(true)
}

/// The `trending` scope: the rows with many views.
fn trending(query: Select<Note>) -> Select<Note> {
    query.filter(Note::VIEWS.gt(10))
}

/// A model with a relation and a named scope: the derive's attributes work
/// together.
#[derive(Debug, Clone, PartialEq, Model)]
#[rok(has_many(posts = Post::AUTHOR_ID), scopes(busy = busy))]
struct Writer {
    #[rok(generated)]
    id: i64,
    name: String,
}

/// The `busy` scope: the writers with a name.
fn busy(query: Select<Writer>) -> Select<Writer> {
    query.filter(Writer::NAME.ne(""))
}

#[test]
fn the_model_entry_starts_a_chain() {
    assert_eq!(
        Visitor::popular().to_sql().as_str(),
        r#"SELECT "id", "name", "age", "views" FROM "visitors" WHERE "views" > $1"#
    );
    assert_eq!(
        Visitor::active().to_sql().as_str(),
        r#"SELECT "id", "name", "age", "views" FROM "visitors" WHERE "age" >= $1"#
    );
}

#[test]
fn the_chain_method_composes_with_the_model_entry() {
    // The second scope receives the chain the first one returned.
    assert_eq!(
        Visitor::popular().active().to_sql().as_str(),
        r#"SELECT "id", "name", "age", "views" FROM "visitors" WHERE "views" > $1 AND "age" >= $2"#
    );
    // And the other way round.
    assert_eq!(
        Visitor::active().popular().to_sql().as_str(),
        r#"SELECT "id", "name", "age", "views" FROM "visitors" WHERE "age" >= $1 AND "views" > $2"#
    );
}

#[test]
fn the_parameterized_scope_takes_its_arguments() {
    // The entry, on the model.
    assert_eq!(
        Visitor::older_than(18).to_sql().as_str(),
        r#"SELECT "id", "name", "age", "views" FROM "visitors" WHERE "age" > $1"#
    );
    // The chain method, with the same shape; it composes.
    assert_eq!(
        Visitor::popular().older_than(17).to_sql().as_str(),
        r#"SELECT "id", "name", "age", "views" FROM "visitors" WHERE "views" > $1 AND "age" > $2"#
    );
}

#[test]
fn a_scope_composes_with_the_rest_of_the_chain() {
    assert_eq!(
        Visitor::popular()
            .filter(Visitor::NAME.eq("ann"))
            .order_by(Visitor::ID)
            .limit(3)
            .to_sql()
            .as_str(),
        r#"SELECT "id", "name", "age", "views" FROM "visitors" WHERE "views" > $1 AND "name" = $2 ORDER BY "id" ASC LIMIT 3"#
    );
}

#[test]
fn the_default_scope_still_applies_alongside_a_scope() {
    assert_eq!(
        Note::trending().to_sql().as_str(),
        r#"SELECT "id", "title", "published", "views" FROM "notes" WHERE "views" > $1 AND "published" = $2"#
    );
    // `unscoped` drops the default scope, and only that one.
    assert_eq!(
        Note::trending().unscoped().to_sql().as_str(),
        r#"SELECT "id", "title", "published", "views" FROM "notes" WHERE "views" > $1"#
    );
}

#[test]
fn a_scope_and_a_relation_live_together() {
    let writer = Writer {
        id: 7,
        name: "ann".into(),
    };
    assert_eq!(
        Writer::busy().to_sql().as_str(),
        r#"SELECT "id", "name" FROM "writers" WHERE "name" <> $1"#
    );
    // The relation still works.
    assert_eq!(
        writer.posts().to_sql().as_str(),
        r#"SELECT "id", "author_id", "category_id", "title", "views" FROM "posts" WHERE "author_id" = $1"#
    );
}

#[tokio::test]
async fn the_scopes_reach_the_database() {
    let Some(db) = setup().await else { return };
    Visitor::insert_all(
        &db,
        &[
            Visitor {
                id: 0,
                name: "ann".into(),
                age: 30,
                views: 500,
            },
            Visitor {
                id: 0,
                name: "bob".into(),
                age: 10,
                views: 500,
            },
            Visitor {
                id: 0,
                name: "cid".into(),
                age: 40,
                views: 5,
            },
        ],
    )
    .await
    .unwrap();

    let popular = Visitor::popular()
        .order_by(Visitor::ID)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(
        popular.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
        ["ann", "bob"]
    );

    // The scopes compose against the database too.
    let both = Visitor::popular()
        .active()
        .order_by(Visitor::ID)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(
        both.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
        ["ann"]
    );
}
