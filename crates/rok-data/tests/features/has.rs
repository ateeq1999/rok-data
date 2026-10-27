//! Filtering rows by their relations: `has`, `where_has`, `doesnt_have`,
//! `where_doesnt_have`, `where_relation`.

use super::*;

// ----- SQL shape -------------------------------------------------------------

#[test]
fn has_is_a_correlated_exists() {
    assert_eq!(
        User::query().has(User::POSTS).to_sql().as_str(),
        concat!(
            r#"SELECT "id", "name", "role", "age" FROM "users" WHERE EXISTS ("#,
            r#"SELECT 1 FROM "posts" WHERE "posts"."author_id" = "users"."id")"#,
        )
    );
    assert_eq!(
        Post::query()
            .where_relation(Post::AUTHOR, User::ROLE.eq("admin"))
            .to_sql()
            .as_str(),
        concat!(
            r#"SELECT "id", "author_id", "category_id", "title", "views" FROM "posts" WHERE EXISTS ("#,
            r#"SELECT 1 FROM "users" WHERE "role" = $1 AND "users"."id" = "posts"."author_id")"#,
        )
    );
}

#[test]
fn doesnt_have_negates_and_the_shape_lands_inside() {
    let sql = User::query()
        .where_doesnt_have(User::POSTS, |q| q.filter(Post::VIEWS.gte(100)))
        .to_sql();
    assert!(
        sql.as_str().ends_with(concat!(
            r#"WHERE NOT EXISTS (SELECT 1 FROM "posts" WHERE "views" >= $1"#,
            r#" AND "posts"."author_id" = "users"."id")"#,
        )),
        "{sql}"
    );
}

// ----- Database --------------------------------------------------------------

#[tokio::test]
async fn relation_filters_pick_the_rows_they_say() {
    let Some(db) = setup().await else { return };
    seed(&db).await;
    let names = |q: Select<User>| {
        let db = db.clone();
        async move {
            let mut names: Vec<String> = q
                .all(&db)
                .await
                .unwrap()
                .into_iter()
                .map(|u| u.name)
                .collect();
            names.sort();
            names
        }
    };
    // Every seeded user has a post; only ann has a profile.
    assert_eq!(
        names(User::query().has(User::POSTS)).await,
        ["ann", "bob", "cid"]
    );
    assert_eq!(names(User::query().has(User::PROFILE)).await, ["ann"]);
    assert_eq!(
        names(User::query().doesnt_have(User::PROFILE)).await,
        ["bob", "cid"]
    );
    assert_eq!(
        names(User::query().where_has(User::POSTS, |q| q.filter(Post::VIEWS.gte(30)))).await,
        ["ann", "cid"]
    );
    assert_eq!(
        names(User::query().where_doesnt_have(User::POSTS, |q| q.filter(Post::VIEWS.gte(30))))
            .await,
        ["bob"]
    );
    // A belongs-to relation: posts by an admin.
    let titles: Vec<String> = Post::query()
        .where_relation(Post::AUTHOR, User::ROLE.eq("admin"))
        .order_by(Post::TITLE)
        .all(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.title)
        .collect();
    assert_eq!(titles, ["a1", "a2", "a3"]);
}

// ----- Aggregates beside the rows --------------------------------------------

#[tokio::test]
async fn aggregates_load_beside_the_rows() {
    let Some(db) = setup().await else { return };
    let (users, _) = seed(&db).await;
    // ann: a1 10, a2 20, a3 30; bob: b1 5, b2 15; cid: c1 100. Only ann
    // has a profile.
    let views = User::POSTS.sum::<i64>(Post::VIEWS);
    let top = User::POSTS.max::<i32>(Post::VIEWS);
    let average = User::POSTS.avg::<f64>(Post::VIEWS);
    let view = User::query()
        .order_by(User::NAME)
        .with_count(User::POSTS)
        .with_exists(User::PROFILE)
        .with(views)
        .with(top)
        .with(average)
        .all(&db)
        .await
        .unwrap();
    let read = |user: &User| {
        (
            view.count(User::POSTS, user),
            view.exists(User::PROFILE, user),
            view.aggregate(views, user),
            view.aggregate(top, user),
            view.aggregate(average, user),
        )
    };
    let [ann, bob, cid] = [&users[0], &users[1], &users[2]];
    assert_eq!(
        read(ann),
        (
            Some(3),
            Some(true),
            Some(60),
            Some(Some(30)),
            Some(Some(20.0))
        )
    );
    assert_eq!(
        read(bob),
        (
            Some(2),
            Some(false),
            Some(20),
            Some(Some(15)),
            Some(Some(10.0))
        )
    );
    assert_eq!(
        read(cid),
        (
            Some(1),
            Some(false),
            Some(100),
            Some(Some(100)),
            Some(Some(100.0))
        )
    );
    // An aggregate the query didn't ask for reads as `None`.
    assert_eq!(view.count(User::PROFILE, ann), None);
}

#[tokio::test]
async fn an_owner_without_records_reads_the_default() {
    let Some(db) = setup().await else { return };
    let lone = User::insert_builder()
        .set(User::NAME, "dee")
        .set(User::ROLE, "member")
        .set(User::AGE, 50)
        .exec(&db)
        .await
        .unwrap();
    let low = User::POSTS.min::<i32>(Post::VIEWS);
    let view = User::filter(User::ID.eq(lone.id))
        .with_count(User::POSTS)
        .with(User::POSTS.sum::<i64>(Post::VIEWS))
        .with(low)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(view.count(User::POSTS, &lone), Some(0));
    assert_eq!(
        view.aggregate(User::POSTS.sum::<i64>(Post::VIEWS), &lone),
        Some(0)
    );
    assert_eq!(view.aggregate(low, &lone), Some(None));
    // A collection loads counts the same way.
    let collection = ModelCollection::from(vec![lone.clone()]);
    let counted = collection.load_count(&db, User::POSTS).await.unwrap();
    assert_eq!(counted.count(User::POSTS, &lone), Some(0));
}

#[tokio::test]
async fn reading_an_unloaded_relation_with_get_names_the_fix() {
    let Some(db) = setup().await else { return };
    let (users, _) = seed(&db).await;
    let view = User::query().with(User::POSTS).all(&db).await.unwrap();
    assert_eq!(view.get(User::POSTS, &users[0]).unwrap().len(), 3);
    let error = view.get(User::PROFILE, &users[0]).unwrap_err();
    assert!(error.is_not_loaded(), "{error}");
    assert!(error.to_string().contains("`.with(…)`"), "{error}");
}
