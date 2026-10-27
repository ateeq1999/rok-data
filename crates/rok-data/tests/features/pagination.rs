//! Offset pagination: `paginate`, `page` at the model's size,
//! `simple_paginate` without a count, `for_page`, and pagination on a
//! relation and with includes.

use super::*;

/// A model with its own page size.
#[derive(Debug, Clone, PartialEq, Model)]
#[rok(per_page = 25)]
struct Ticket {
    id: i64,
    title: String,
}

#[test]
fn a_model_carries_its_page_size() {
    assert_eq!(User::PER_PAGE, 15, "the default");
    assert_eq!(Ticket::PER_PAGE, 25);
}

#[test]
fn for_page_is_the_window_as_chain_sugar() {
    assert_eq!(
        User::query()
            .order_by(User::ID)
            .for_page(3, 10)
            .to_sql()
            .as_str(),
        User::query()
            .order_by(User::ID)
            .limit(10)
            .offset(20)
            .to_sql()
            .as_str()
    );
    // Page 0 reads as page 1.
    assert_eq!(
        User::query().for_page(0, 10).to_sql().as_str(),
        User::query().limit(10).offset(0).to_sql().as_str()
    );
}

#[tokio::test]
async fn simple_paginate_knows_whether_more_follow_without_a_count() {
    let Some(db) = setup().await else { return };
    seed(&db).await; // six posts

    let first = Post::query()
        .order_by(Post::ID)
        .simple_paginate(&db, 1, 4)
        .await
        .unwrap();
    assert_eq!((first.len(), first.has_more()), (4, true));
    assert!(first.on_first_page());

    let second = Post::query()
        .order_by(Post::ID)
        .simple_paginate(&db, 2, 4)
        .await
        .unwrap();
    assert_eq!((second.len(), second.has_more()), (2, false));

    let past = Post::query().simple_paginate(&db, 9, 4).await.unwrap();
    assert!(past.is_empty() && !past.has_more());
    assert!(
        Post::query()
            .simple_paginate(&db, 1, 0)
            .await
            .unwrap_err()
            .to_string()
            .contains("per_page")
    );
}

#[tokio::test]
async fn page_paginates_at_the_models_size() {
    let Some(db) = setup().await else { return };
    seed(&db).await;
    let page = User::query().page(&db, 1).await.unwrap();
    assert_eq!((page.len(), page.per_page, page.total), (3, 15, 3));
    assert!(!page.has_pages());
}

#[tokio::test]
async fn a_relation_paginates_like_any_chain() {
    let Some(db) = setup().await else { return };
    let (users, posts) = seed(&db).await;
    let author = &users[0];
    let theirs = posts.iter().filter(|p| p.author_id == author.id).count() as u64;
    let page = author
        .posts()
        .order_by(Post::ID)
        .paginate(&db, 1, 1)
        .await
        .unwrap();
    assert_eq!((page.len(), page.total), (1, theirs));
    assert!(page.items.iter().all(|post| post.author_id == author.id));
}

#[tokio::test]
async fn a_with_query_paginates_and_loads_its_includes_for_that_page() {
    let Some(db) = setup().await else { return };
    seed(&db).await;
    let page = Post::query()
        .order_by(Post::ID)
        .with(Post::AUTHOR)
        .paginate(&db, 2, 4)
        .await
        .unwrap();
    assert_eq!((page.rows().len(), page.total, page.page), (2, 6, 2));
    assert!(!page.has_next());
    for post in page.rows() {
        let author = page.try_get(Post::AUTHOR, post).unwrap();
        assert_eq!(author.map(|user| user.id), Some(post.author_id));
    }
    assert_eq!(page.into_page().last_item(), Some(6));
}
