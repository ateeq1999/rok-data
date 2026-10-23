//! The model collection: the entries on the model, the key verbs, the
//! loads beside already-fetched rows, and the fresh re-read.

use super::*;

#[test]
fn to_collection_vec_carries_rows_in_order() {
    let users = vec![
        User {
            id: 7,
            name: "ann".into(),
            role: "admin".into(),
            age: 30,
        },
        User {
            id: 9,
            name: "bob".into(),
            role: "member".into(),
            age: 20,
        },
    ];
    let collection = User::to_collection_vec(&users);
    assert_eq!(collection.len(), 2);
    // The row-level entry.
    assert_eq!(collection[0].to_collection().len(), 1);
    // Key verbs.
    assert_eq!(
        collection.find(9).map(|user| user.name.as_str()),
        Some("bob")
    );
    assert!(collection.contains(7));
    assert!(!collection.contains(8));
    assert_eq!(
        collection.model_keys(),
        vec![rok_data::Value::from(7i64), rok_data::Value::from(9i64)]
    );
    assert!(collection.find_or_fail(8).is_err());
    // Set verbs, by key.
    assert_eq!(collection.only(vec![9]).len(), 1);
    assert_eq!(User::to_collection_vec(&users).except(vec![7]).len(), 1);
    let kept = User::to_collection_vec(&users);
    let other = User::to_collection_vec(&[users[0].clone()]);
    assert_eq!(kept.diff(&other).len(), 1);
    assert_eq!(other.intersect(&User::to_collection_vec(&users)).len(), 1);
    // unique keeps the first of every key.
    let doubles = User::to_collection_vec(&[users[0].clone(), users[0].clone(), users[1].clone()]);
    assert_eq!(doubles.unique().len(), 2);
}

#[test]
fn to_query_selects_by_the_rows_keys() {
    let users = vec![
        User {
            id: 7,
            name: "ann".into(),
            role: "admin".into(),
            age: 30,
        },
        User {
            id: 9,
            name: "bob".into(),
            role: "member".into(),
            age: 20,
        },
    ];
    assert_eq!(
        User::to_collection_vec(&users).to_query().to_sql().as_str(),
        r#"SELECT "id", "name", "role", "age" FROM "users" WHERE "id" IN ($1, $2)"#
    );
    // An empty collection matches nothing.
    assert!(
        User::to_collection_vec(&[])
            .to_query()
            .to_sql()
            .as_str()
            .ends_with(r#"WHERE FALSE"#)
    );
}

#[tokio::test]
async fn collection_load_fills_the_relations() {
    let Some(db) = setup().await else { return };
    let (users, _) = seed(&db).await;
    let posts = Post::query().all(&db).await.expect("posts");
    let posts = Post::to_collection_vec(&posts);
    assert_eq!(posts.len(), 6);
    let mut view = posts.load(&db, [Post::AUTHOR]).await.expect("load");

    // The first post belongs to `ann`; the author is loaded.
    let post = view.rows().first().expect("rows").clone();
    let author = view
        .try_get(Post::AUTHOR, &post)
        .expect("author loaded")
        .expect("author present");
    assert_eq!(
        author.id,
        users
            .iter()
            .find(|user| post.author_id == user.id)
            .expect("user")
            .id
    );
    view.load_missing(&db, [Post::CATEGORY])
        .await
        .expect("missing");
    assert!(view.loaded(Post::CATEGORY).is_some());
}

#[tokio::test]
async fn load_missing_does_not_refetch_a_parked_include() {
    let Some(db) = setup().await else { return };
    let (users, _) = seed(&db).await;
    // The admin fetch: one user, one post by the include's batch cap.
    let mut view = User::filter(User::ROLE.eq("admin"))
        .with(Include::new(User::POSTS).limit(1))
        .all(&db)
        .await
        .expect("query");
    assert_eq!(
        view.try_get(User::POSTS, &users[0]).expect("parked").len(),
        1
    );
    // Asking for the posts again is a no-op: the shaping stays.
    view.load_missing(&db, [Include::new(User::POSTS).limit(99)])
        .await
        .expect("load_missing");
    assert_eq!(
        view.try_get(User::POSTS, &users[0]).expect("parked").len(),
        1,
        "the first include's shaping stands"
    );
    // A still-missing include runs.
    view.load_missing(&db, [User::PROFILE])
        .await
        .expect("load_missing");
    assert!(view.loaded(User::PROFILE).is_some());
}

#[tokio::test]
async fn fresh_rereads_the_rows_through_their_keys() {
    let Some(db) = setup().await else { return };
    let (users, _) = seed(&db).await;
    // Change what the database knows about `ann`.
    let mut ann = users[0].clone();
    ann.name = "anna".into();
    ann.update(&db).await.expect("update");
    let fresh = User::to_collection_vec(&users)
        .fresh(&db)
        .await
        .expect("fresh");
    assert_eq!(fresh.len(), 3);
    assert_eq!(
        fresh.find(ann.id).map(|user| user.name.as_str()),
        Some("anna"),
        "the fresh read sees the new name"
    );
    // The re-read applies the model's own scopes: a soft deleted row
    // drops out exactly as any other query drops it (here: none).
    assert!(fresh.contains(users[2].id));
}
