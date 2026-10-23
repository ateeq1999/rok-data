//! The model's surface: the verbs 0.3.0 renames and the ones it adds.

use super::*;

/// A table of its own, to ask "is there a row?" about an empty table.
#[derive(Debug, Clone, PartialEq, Model)]
struct Tag {
    #[rok(generated)]
    id: i64,
    label: String,
}

// ----- SQL shape -------------------------------------------------------------

/// `where_` is the Eloquent spelling of `filter`, on the model and on the
/// chain: the same statement, the same parameters.
#[test]
fn where_is_an_alias_of_filter() {
    assert_eq!(
        User::where_(User::ROLE.eq("admin")).to_sql().as_str(),
        User::filter(User::ROLE.eq("admin")).to_sql().as_str(),
    );
    assert_eq!(
        User::where_(User::ROLE.eq("admin")).to_sql().as_str(),
        r#"SELECT "id", "name", "role", "age" FROM "users" WHERE "role" = $1"#,
    );
    assert_eq!(
        User::query()
            .where_(User::ROLE.eq("admin"))
            .to_sql()
            .as_str(),
        User::query()
            .filter(User::ROLE.eq("admin"))
            .to_sql()
            .as_str(),
    );
    assert_eq!(
        User::query()
            .where_(User::ROLE.eq("admin"))
            .to_sql()
            .as_str(),
        r#"SELECT "id", "name", "role", "age" FROM "users" WHERE "role" = $1"#,
    );
}

/// The insert builder is the model's `insert_builder` now; the statement it
/// writes is the one `create` wrote.
#[test]
fn the_insert_builder_writes_the_same_statement() {
    assert_eq!(
        User::insert_builder()
            .set(User::NAME, "ann")
            .set(User::ROLE, "admin")
            .to_sql()
            .as_str(),
        r#"INSERT INTO "users" ("name", "role") VALUES ($1, $2) RETURNING "id", "name", "role", "age""#,
    );
}

/// The statement `chunk` streams: the chain's own, in one query, with
/// nothing added to it for the windowing.
#[test]
fn chunk_streams_the_chains_statement() {
    let chunked = Post::query().order_by(Post::ID);
    assert_eq!(
        chunked.to_sql().as_str(),
        r#"SELECT "id", "author_id", "category_id", "title", "views" FROM "posts" ORDER BY "id" ASC"#,
    );
}

// ----- against a database ----------------------------------------------------

/// The renamed verbs do what the old ones did.
#[tokio::test]
async fn the_renamed_verbs_behave_as_before() {
    let Some(db) = setup().await else { return };

    // `insert_builder` inserts a row, as `create` did.
    let dee = User::insert_builder()
        .set(User::NAME, "dee")
        .set(User::ROLE, "member")
        .set(User::AGE, 44)
        .exec(&db)
        .await
        .unwrap();
    assert_eq!((dee.name.as_str(), dee.age), ("dee", 44));

    // `update` writes every column and returns the stored row.
    let mut edited = dee.clone();
    edited.name = "dee2".into();
    edited.age = 45;
    let stored = edited.update(&db).await.unwrap();
    assert_eq!((stored.name.as_str(), stored.age), ("dee2", 45));
    assert_eq!(dee.reload(&db).await.unwrap().name, "dee2");

    // `update_only` writes only the listed columns.
    let mut renamed = stored.clone();
    renamed.name = "dee3".into();
    renamed.age = 46;
    let stored = renamed
        .update_only(&db, Some(vec![User::NAME]))
        .await
        .unwrap();
    assert_eq!(stored.name.as_str(), "dee3");
    assert_eq!(stored.age, 45, "the other column is left alone");
}

/// The model's `exists` is the chain's `exists` over the whole table.
#[tokio::test]
async fn the_model_exists_agrees_with_the_chain() {
    let Some(db) = setup().await else { return };
    db.execute("CREATE TEMP TABLE tags (id BIGSERIAL PRIMARY KEY, label TEXT NOT NULL)")
        .await
        .unwrap();

    // An empty table: no row, no existence.
    assert!(!Tag::exists(&db).await.unwrap());
    assert_eq!(
        Tag::exists(&db).await.unwrap(),
        Tag::query().exists(&db).await.unwrap(),
    );

    // A table with a row.
    Tag::insert_builder()
        .set(Tag::LABEL, "rust")
        .exec(&db)
        .await
        .unwrap();
    assert!(Tag::exists(&db).await.unwrap());
    assert_eq!(
        Tag::exists(&db).await.unwrap(),
        Tag::query().exists(&db).await.unwrap(),
    );

    // The chain's `exists` sees the filter; the model's sees the table.
    assert!(!Tag::filter(Tag::LABEL.eq("go")).exists(&db).await.unwrap());
    assert!(Tag::exists(&db).await.unwrap());
}

/// `returning_one_or_fail` returns the stored row, and a key no row has is
/// `NotFound` — the one convention for verbs that target a row by key.
#[tokio::test]
async fn returning_one_or_fail_is_not_found_for_a_missing_key() {
    let Some(db) = setup().await else { return };
    let (users, _) = seed(&db).await;

    let renamed = User::update_all()
        .set(User::NAME, "ada")
        .filter(User::ID.eq(users[0].id))
        .returning_one_or_fail(&db)
        .await
        .unwrap();
    assert_eq!(renamed.name, "ada");

    let missing = User::update_all()
        .set(User::NAME, "nobody")
        .filter(User::ID.eq(i64::MAX))
        .returning_one_or_fail(&db)
        .await
        .unwrap_err();
    assert!(missing.is_not_found(), "{missing}");
}

/// `get` returns the rows `all` returns, as a `ModelCollection`.
#[tokio::test]
async fn get_returns_what_all_returns_as_a_collection() {
    let Some(db) = setup().await else { return };
    seed(&db).await;

    let all = User::query().order_by(User::ID).all(&db).await.unwrap();
    let got = User::query().order_by(User::ID).get(&db).await.unwrap();
    assert_eq!(all, *got, "the same rows; `get` wraps them in a collection");
    assert_eq!(all.len(), 3);

    // The model's `all` and the chain's `get` agree.
    let mut model_all = User::all(&db).await.unwrap();
    let mut chain_get = User::query().get(&db).await.unwrap();
    model_all.sort_by_key(|u| u.id);
    chain_get.sort_by_key(|u| u.id);
    assert_eq!(model_all, *chain_get);
}

/// `where_` filters exactly like `filter`, on the model and on the chain.
#[tokio::test]
async fn where_filters_like_filter() {
    let Some(db) = setup().await else { return };
    seed(&db).await;

    // The model's.
    let admins = User::filter(User::ROLE.eq("admin")).all(&db).await.unwrap();
    let admins_where = User::where_(User::ROLE.eq("admin")).all(&db).await.unwrap();
    assert_eq!(admins, admins_where);
    assert_eq!(admins.len(), 1);
    assert_eq!(admins[0].name, "ann");

    // The chain's.
    let popular = Post::filter(Post::VIEWS.gt(15))
        .order_by(Post::ID)
        .all(&db)
        .await
        .unwrap();
    let popular_where = Post::query()
        .where_(Post::VIEWS.gt(15))
        .order_by(Post::ID)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(popular, popular_where);
    assert_eq!(popular.len(), 3);
}

/// `chunk` hands every row to the callback exactly once, in the query's
/// order, in windows of the requested size.
#[tokio::test]
async fn chunk_visits_every_row_once_in_windows() {
    let Some(db) = setup().await else { return };
    seed(&db).await;

    // Six posts, windows of two: three windows of two.
    let mut windows: Vec<Vec<String>> = Vec::new();
    Post::order_by(Post::ID)
        .chunk(&db, 2, |rows| {
            windows.push(rows.iter().map(|p| p.title.clone()).collect());
            async { Ok(()) }
        })
        .await
        .unwrap();
    assert_eq!(
        windows.iter().map(Vec::len).collect::<Vec<_>>(),
        [2, 2, 2],
        "the window size is honoured"
    );

    // Every row, once, in the order the query returns them.
    let all: Vec<String> = Post::order_by(Post::ID)
        .all(&db)
        .await
        .unwrap()
        .iter()
        .map(|p| p.title.clone())
        .collect();
    let visited: Vec<String> = windows.into_iter().flatten().collect();
    assert_eq!(visited, all);

    // Three users, windows of two: the last window is shorter.
    let mut sizes = Vec::new();
    User::order_by(User::ID)
        .chunk(&db, 2, |rows| {
            sizes.push(rows.len());
            async { Ok(()) }
        })
        .await
        .unwrap();
    assert_eq!(sizes, [2, 1]);

    // A window that is larger than the table is one window.
    let mut sizes = Vec::new();
    User::order_by(User::ID)
        .chunk(&db, 100, |rows| {
            sizes.push(rows.len());
            async { Ok(()) }
        })
        .await
        .unwrap();
    assert_eq!(sizes, [3]);

    // A window of zero is refused.
    let err = User::query()
        .chunk(&db, 0, |_rows| async { Ok(()) })
        .await
        .unwrap_err();
    assert!(err.to_string().contains("size"), "{err}");
}
