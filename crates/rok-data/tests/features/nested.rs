//! Nested includes: a relation's records get their own relations loaded,
//! one batched query per level.

use super::*;

#[tokio::test]
async fn a_nested_include_loads_on_the_relations_records() {
    let Some(db) = setup().await else { return };
    let (users, _) = seed(&db).await;
    let ann = &users[0];
    let view = User::query()
        .with(User::POSTS.with(Post::CATEGORY))
        .all(&db)
        .await
        .unwrap();
    let posts = view.nested(User::POSTS).expect("nested view");
    // Every post the relation loaded, as rows of the nested view.
    assert_eq!(posts.rows().len(), 6);
    let mut categories: Vec<(String, Option<String>)> = view
        .get(User::POSTS, ann)
        .unwrap()
        .iter()
        .map(|post| {
            let category = posts.get(Post::CATEGORY, post).unwrap();
            (post.title.clone(), category.map(|c| c.name.clone()))
        })
        .collect();
    categories.sort();
    assert_eq!(
        categories,
        [
            ("a1".to_owned(), Some("rust".to_owned())),
            ("a2".to_owned(), Some("sql".to_owned())),
            ("a3".to_owned(), None),
        ]
    );
    // A relation without nested includes has no nested view.
    let flat = User::query().with(User::POSTS).all(&db).await.unwrap();
    assert!(flat.nested(User::POSTS).is_none());
}

#[tokio::test]
async fn nesting_goes_through_a_belongs_to_and_deeper() {
    let Some(db) = setup().await else { return };
    seed(&db).await;
    // posts → author → profile, and the author's posts → category.
    let view = Post::query()
        .order_by(Post::TITLE)
        .with(
            Post::AUTHOR
                .with(User::PROFILE)
                .with(User::POSTS.with(Post::CATEGORY)),
        )
        .all(&db)
        .await
        .unwrap();
    let authors = view.nested(Post::AUTHOR).expect("authors");
    let first = &view.rows()[0];
    assert_eq!(first.title, "a1");
    let author = view.get(Post::AUTHOR, first).unwrap().expect("ann");
    let bio = authors
        .get(User::PROFILE, author)
        .unwrap()
        .map(|p| p.bio.as_str());
    assert_eq!(bio, Some("hi"));
    let their_posts = authors.nested(User::POSTS).expect("deeper");
    let titles: Vec<&str> = authors
        .get(User::POSTS, author)
        .unwrap()
        .iter()
        .filter(|post| their_posts.get(Post::CATEGORY, post).unwrap().is_some())
        .map(|post| post.title.as_str())
        .collect();
    assert_eq!(titles.len(), 2, "a1 and a2 have a category: {titles:?}");
}

#[tokio::test]
async fn load_replaces_and_load_missing_keeps() {
    let Some(db) = setup().await else { return };
    let (users, _) = seed(&db).await;
    let ann = &users[0];
    let mut view = User::query().with(User::POSTS).all(&db).await.unwrap();
    assert_eq!(view.get(User::POSTS, ann).unwrap().len(), 3);
    // load_missing leaves the parked include as it is…
    view.load_missing(&db, [Include::new(User::POSTS).filter(Post::VIEWS.gte(30))])
        .await
        .unwrap();
    assert_eq!(view.get(User::POSTS, ann).unwrap().len(), 3);
    // …load replaces it with the new shaping.
    view.load(&db, [Include::new(User::POSTS).filter(Post::VIEWS.gte(30))])
        .await
        .unwrap();
    assert_eq!(view.get(User::POSTS, ann).unwrap().len(), 1);
}
