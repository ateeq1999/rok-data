//! The relations: lazy and eager loading, and the `with` query.

use super::*;

#[tokio::test]
async fn relations_lazy_and_eager() {
    let Some(db) = setup().await else { return };
    let (users, posts) = seed(&db).await;

    // Lazy
    let ann_posts = users[0]
        .posts()
        .order_by(Post::TITLE)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(
        ann_posts
            .iter()
            .map(|p| p.title.as_str())
            .collect::<Vec<_>>(),
        ["a1", "a2", "a3"]
    );
    assert_eq!(posts[5].author().one(&db).await.unwrap().name, "cid");
    assert!(posts[2].category().first(&db).await.unwrap().is_none());
    assert_eq!(users[0].profile().one(&db).await.unwrap().bio, "hi");

    // Eager: has_many
    let by_author = User::POSTS.load(&db, &users).await.unwrap();
    assert_eq!(by_author.len(), 6);
    assert_eq!(
        users
            .iter()
            .map(|u| by_author.get(u).len())
            .collect::<Vec<_>>(),
        [3, 2, 1]
    );
    let top = User::POSTS
        .load_from(
            &db,
            &users,
            Post::filter(Post::VIEWS.gte(15)).order_by(Post::VIEWS.desc()),
        )
        .await
        .unwrap();
    assert_eq!(
        top.get(&users[0])
            .iter()
            .map(|p| p.views)
            .collect::<Vec<_>>(),
        [30, 20]
    );

    // Eager: belongs_to, with NULL and shared foreign keys
    let mut categories = Post::CATEGORY.load(&db, &posts).await.unwrap();
    assert_eq!(categories.len(), 2);
    assert_eq!(categories.get(&posts[0]).unwrap().name, "rust");
    assert!(categories.get(&posts[2]).is_none());
    assert_eq!(categories.take(&posts[5]).unwrap().name, "sql");
    let authors = Post::AUTHOR.load(&db, &posts).await.unwrap();
    assert!(
        posts
            .iter()
            .all(|p| authors.get(p).unwrap().id == p.author_id)
    );

    // Eager: has_one
    let profiles = User::PROFILE.load(&db, &users).await.unwrap();
    assert!(profiles.get(&users[0]).is_some() && profiles.get(&users[1]).is_none());

    // Empty input runs no query.
    assert!(User::POSTS.load(&db, &[]).await.unwrap().is_empty());
}

#[tokio::test]
async fn with_loads_relations_beside_the_rows() {
    let Some(db) = setup().await else { return };
    let (users, _posts) = seed(&db).await;

    // Rows plus two relations: two batched follow-ups, one view.
    let view = User::query()
        .with(User::POSTS)
        .with(User::PROFILE)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(view.rows().len(), 3);
    assert!(
        view.rows()
            .iter()
            .all(|u| ["ann", "bob", "cid"].contains(&u.name.as_str())),
        "the rows are still plain users"
    );

    // The loaded state, typed and keyed, read like the loaders' maps.
    let posts_by_author = view.loaded(User::POSTS).unwrap();
    assert_eq!(
        users
            .iter()
            .map(|u| posts_by_author.get(u).len())
            .sum::<usize>(),
        6
    );

    // The fused read for one owner.
    let ann_posts = view.try_get(User::POSTS, &users[0]).unwrap();
    assert_eq!(ann_posts.len(), 3);
    assert_eq!(
        ann_posts
            .iter()
            .map(|p| p.title.as_str())
            .collect::<Vec<_>>(),
        ["a1", "a2", "a3"]
    );

    // `One` reads `Option<Option<..>>`: the outer `Some` says the include
    // ran; the inner says whether the owner has the record.
    assert_eq!(
        view.try_get(User::PROFILE, &users[0]).unwrap().unwrap().bio,
        "hi"
    );
    assert!(view.try_get(User::PROFILE, &users[1]).unwrap().is_none());

    // A relation never asked for reads `None` — never panics.
    let slim = User::query().with(User::POSTS).all(&db).await.unwrap();
    assert_eq!(slim.rows().len(), 3);
    assert!(slim.loaded(User::PROFILE).is_none());

    // Shaped include: filters and ordering act on the batched load.
    let shaped = User::query()
        .with(
            Include::new(User::POSTS)
                .filter(Post::VIEWS.gte(15))
                .order_by(Post::VIEWS.desc()),
        )
        .all(&db)
        .await
        .unwrap();
    assert_eq!(shaped.try_get(User::POSTS, &users[0]).unwrap().len(), 2);
    assert_eq!(shaped.try_get(User::POSTS, &users[1]).unwrap().len(), 1);
    assert_eq!(shaped.try_get(User::POSTS, &users[2]).unwrap().len(), 1);

    // Rows and loads share one connection — inside a transaction too.
    db.transaction(|tx| {
        Box::pin(async move {
            let review = User::query()
                .order_by(User::NAME)
                .with(User::POSTS)
                .all(&mut *tx)
                .await
                .unwrap();
            assert_eq!(review.rows().len(), 3);
            assert_eq!(
                review
                    .try_get(User::POSTS, &review.rows()[0])
                    .unwrap()
                    .len(),
                3
            );
            Ok::<(), rok_data::Error>(())
        })
    })
    .await
    .unwrap();

    // Include-free queries keep their old shape (no view, no extra query).
    let plain = User::query().limit(1).all(&db).await.unwrap();
    assert_eq!(plain.len(), 1);
}
