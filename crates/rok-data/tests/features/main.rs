//! Relations, streaming, aggregates, timestamps and memoization.
//!
//! SQL-shape tests always run; database tests need `DATABASE_URL` (see
//! `tests/postgres.rs`).

use std::time::Duration;

use rok_data::prelude::*;
use rok_data::{Include, raw};

mod backstop;
mod collection;
mod conventions;
mod database_rules;
mod encrypted;
mod has;
mod input;
mod input_shapes;
mod lifecycle;
mod many_to_many;
mod memoized;
mod nested;
mod pagination;
mod query_sugar;
mod relations;
mod scopes;
mod streaming;
mod surface;
mod through;
mod timestamps;
mod value_objects;
mod walking;

#[derive(Debug, Clone, PartialEq, Model)]
#[rok(has_many(posts = Post::AUTHOR_ID), has_one(profile = Profile::USER_ID))]
struct User {
    #[rok(generated)]
    id: i64,
    name: String,
    role: String,
    age: i32,
}

#[derive(Debug, Clone, PartialEq, Model)]
struct Post {
    #[rok(generated)]
    id: i64,
    #[rok(belongs_to(author = User))]
    author_id: i64,
    #[rok(belongs_to = Category)]
    category_id: Option<i64>,
    title: String,
    views: i32,
}

#[derive(Debug, Clone, PartialEq, Model)]
#[rok(table = "categories")]
struct Category {
    #[rok(generated)]
    id: i64,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Model)]
struct Profile {
    #[rok(generated)]
    id: i64,
    user_id: i64,
    bio: String,
}

const SCHEMA: &str = r#"
CREATE TEMP TABLE users (
    id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL, role TEXT NOT NULL, age INT NOT NULL
);
CREATE TEMP TABLE categories (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL);
CREATE TEMP TABLE posts (
    id BIGSERIAL PRIMARY KEY,
    author_id BIGINT NOT NULL REFERENCES users (id),
    category_id BIGINT REFERENCES categories (id),
    title TEXT NOT NULL,
    views INT NOT NULL DEFAULT 0
);
CREATE TEMP TABLE profiles (
    id BIGSERIAL PRIMARY KEY, user_id BIGINT NOT NULL REFERENCES users (id), bio TEXT NOT NULL
);
CREATE TEMP TABLE visitors (
    id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL, age INT NOT NULL, views INT NOT NULL DEFAULT 0
);
"#;

async fn setup_with(builder: rok_data::DbBuilder) -> Option<Db> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping");
        return None;
    };
    let db = builder
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect");
    db.execute(SCHEMA).await.expect("schema");
    Some(db)
}

async fn setup() -> Option<Db> {
    setup_with(Db::builder()).await
}

/// 3 users, 2 categories, 6 posts, 1 profile.
async fn seed(db: &Db) -> (Vec<User>, Vec<Post>) {
    let users = User::insert_all(
        db,
        &[
            ("ann", "admin", 30),
            ("bob", "member", 20),
            ("cid", "member", 40),
        ]
        .map(|(name, role, age)| User {
            id: 0,
            name: name.into(),
            role: role.into(),
            age,
        }),
    )
    .await
    .unwrap();
    let rust = Category::insert_builder()
        .set(Category::NAME, "rust")
        .exec(db)
        .await
        .unwrap();
    let sql = Category::insert_builder()
        .set(Category::NAME, "sql")
        .exec(db)
        .await
        .unwrap();
    let post = |author: &User, category: Option<&Category>, title: &str, views| Post {
        id: 0,
        author_id: author.id,
        category_id: category.map(|c| c.id),
        title: title.into(),
        views,
    };
    let posts = Post::insert_all(
        db,
        &[
            post(&users[0], Some(&rust), "a1", 10),
            post(&users[0], Some(&sql), "a2", 20),
            post(&users[0], None, "a3", 30),
            post(&users[1], Some(&rust), "b1", 5),
            post(&users[1], Some(&rust), "b2", 15),
            post(&users[2], Some(&sql), "c1", 100),
        ],
    )
    .await
    .unwrap();
    Profile::insert_builder()
        .set(Profile::USER_ID, users[0].id)
        .set(Profile::BIO, "hi")
        .exec(db)
        .await
        .unwrap();
    (users, posts)
}

// ----- SQL shape -------------------------------------------------------------
#[test]
fn relation_queries_sql() {
    let user = User {
        id: 7,
        name: "x".into(),
        role: "r".into(),
        age: 1,
    };
    assert_eq!(
        user.posts().order_by(Post::ID).to_sql().as_str(),
        r#"SELECT "id", "author_id", "category_id", "title", "views" FROM "posts" WHERE "author_id" = $1 ORDER BY "id" ASC"#
    );
    let post = Post {
        id: 1,
        author_id: 7,
        category_id: None,
        title: "t".into(),
        views: 0,
    };
    assert!(
        post.author()
            .to_sql()
            .as_str()
            .ends_with(r#"FROM "users" WHERE "id" = $1"#)
    );
    assert!(
        post.category().to_sql().as_str().ends_with("WHERE FALSE"),
        "NULL foreign key"
    );
    assert_eq!(Post::AUTHOR.foreign_key().name(), "author_id");
    assert_eq!(User::POSTS.foreign_key().name(), "author_id");
    assert_eq!(User::PROFILE.foreign_key().name(), "user_id");
    assert_eq!(post.value_of("title"), Some(rok_data::Value::from("t")));
    assert_eq!(post.value_of("nope"), None);
}

#[test]
fn projection_sql() {
    let sql = Post::filter(Post::VIEWS.gt(1))
        .group_by(Post::AUTHOR_ID)
        .having(Projection::count_all().gte(2))
        .order_by(Post::AUTHOR_ID)
        .select((
            Post::AUTHOR_ID,
            Projection::count_all(),
            Post::VIEWS.sum().cast("BIGINT").alias("total"),
        ))
        .to_sql();
    assert_eq!(
        sql.as_str(),
        r#"SELECT "author_id", COUNT(*), CAST(SUM("views") AS BIGINT) AS "total" FROM "posts" WHERE "views" > $1 GROUP BY "author_id" HAVING COUNT(*) >= $2 ORDER BY "author_id" ASC"#
    );
}
