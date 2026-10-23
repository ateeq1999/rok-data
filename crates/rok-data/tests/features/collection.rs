//! The model collection: the key verbs, the set verbs, the query back to
//! the rows, the loads beside already-fetched rows, and the fresh re-read.

use super::*;

/// Two users: `ann` (7, admin) and `bob` (9, member).
fn ann_and_bob() -> Vec<User> {
    vec![
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
    ]
}

/// A model with a composite primary key.
#[derive(Debug, Clone, PartialEq, Model)]
struct Membership {
    #[rok(primary_key)]
    team_id: i64,
    #[rok(primary_key)]
    user_id: i64,
    role: String,
}

fn memberships() -> ModelCollection<Membership> {
    [(1, 7, "lead"), (1, 9, "member"), (2, 7, "member")]
        .into_iter()
        .map(|(team_id, user_id, role)| Membership {
            team_id,
            user_id,
            role: role.into(),
        })
        .collect()
}

#[test]
fn the_key_verbs_find_rows_by_their_keys() {
    let users = ModelCollection::from(ann_and_bob());
    assert_eq!(users.len(), 2, "the Vec shape, through Deref");
    assert_eq!(users.find(9).map(|user| user.name.as_str()), Some("bob"));
    assert!(users.contains_key(7));
    assert!(!users.contains_key(8));
    // The slice's own `contains(&row)` is not shadowed.
    assert!(users.contains(&ann_and_bob()[0]));
    assert_eq!(
        users.model_keys(),
        vec![
            vec![rok_data::Value::from(7i64)],
            vec![rok_data::Value::from(9i64)]
        ]
    );
    assert!(users.find_or_fail(8).unwrap_err().is_not_found());
}

#[test]
fn the_set_verbs_compare_by_key_and_keep_order() {
    let users = || ModelCollection::from(ann_and_bob());
    assert_eq!(users().only(vec![9]).len(), 1);
    assert_eq!(users().except(vec![7]).len(), 1);
    let ann: ModelCollection<User> = ann_and_bob().into_iter().take(1).collect();
    assert_eq!(users().diff(&ann).len(), 1);
    assert_eq!(ann.clone().intersect(&users()).len(), 1);
    // `unique` keeps the first of every key.
    let doubles: ModelCollection<User> = [0, 0, 1].map(|i| ann_and_bob()[i].clone()).into();
    assert_eq!(doubles.unique().len(), 2);
    // `partition` splits by a predicate, both halves in order.
    let (admins, rest) = users().partition(|user| user.role == "admin");
    assert_eq!(admins.model_keys(), vec![vec![rok_data::Value::from(7i64)]]);
    assert_eq!(rest.model_keys(), vec![vec![rok_data::Value::from(9i64)]]);
}

#[test]
fn composite_keys_are_whole_in_every_verb() {
    let rows = memberships();
    assert_eq!(rows.find((1, 9)).map(|m| m.role.as_str()), Some("member"));
    assert!(rows.contains_key((2, 7)));
    assert!(!rows.contains_key((2, 9)));
    assert_eq!(
        rows.model_keys()[2],
        vec![rok_data::Value::from(2i64), rok_data::Value::from(7i64)],
        "every key column, in key order"
    );
    // One condition per row, every row present.
    let sql = rows.to_query().to_sql();
    assert_eq!(
        sql.as_str(),
        r#"SELECT "team_id", "user_id", "role" FROM "memberships" WHERE (("team_id" = $1 AND "user_id" = $2) OR ("team_id" = $3 AND "user_id" = $4) OR ("team_id" = $5 AND "user_id" = $6))"#
    );
}

#[test]
fn to_query_selects_by_the_rows_keys() {
    assert_eq!(
        ModelCollection::from(ann_and_bob())
            .to_query()
            .to_sql()
            .as_str(),
        r#"SELECT "id", "name", "role", "age" FROM "users" WHERE "id" IN ($1, $2)"#
    );
    // An empty collection matches nothing.
    assert!(
        ModelCollection::<User>::from(Vec::new())
            .to_query()
            .to_sql()
            .as_str()
            .ends_with(r#"WHERE FALSE"#)
    );
}

#[cfg(feature = "json")]
#[test]
fn a_collection_serializes_as_the_array_of_its_rows() {
    #[derive(Debug, Clone, PartialEq, Model, serde::Serialize)]
    struct Tag {
        id: i64,
        label: String,
    }
    let tags: ModelCollection<Tag> = [(1, "a"), (2, "b")]
        .map(|(id, label)| Tag {
            id,
            label: label.into(),
        })
        .into();
    let expected = r#"[{"id":1,"label":"a"},{"id":2,"label":"b"}]"#;
    assert_eq!(tags.to_json().unwrap(), expected);
    assert_eq!(serde_json::to_string(&tags).unwrap(), expected);
}

#[tokio::test]
async fn get_returns_a_collection_the_verbs_work_on() {
    let Some(db) = setup().await else { return };
    let (users, _) = seed(&db).await;
    let got = User::query().order_by(User::ID).get(&db).await.unwrap();
    assert_eq!(
        *got,
        User::query().order_by(User::ID).all(&db).await.unwrap()
    );
    // The headline call: `get`, then a collection verb, no conversion.
    assert_eq!(
        got.find(users[1].id).map(|user| user.name.as_str()),
        Some("bob")
    );
}

#[tokio::test]
async fn collection_load_fills_the_relations() {
    let Some(db) = setup().await else { return };
    let (users, _) = seed(&db).await;
    let posts = Post::query().get(&db).await.expect("posts");
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
async fn a_collection_loads_missing_includes_like_the_loaded_view() {
    let Some(db) = setup().await else { return };
    seed(&db).await;
    let posts = Post::query().get(&db).await.expect("posts");
    // Nothing is loaded on bare rows, so `load_missing` loads it all.
    let view = posts
        .load_missing(&db, [Post::AUTHOR])
        .await
        .expect("load_missing");
    assert!(view.loaded(Post::AUTHOR).is_some());
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
    let fresh = ModelCollection::from(users.clone())
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
    assert!(fresh.contains_key(users[2].id));
}
