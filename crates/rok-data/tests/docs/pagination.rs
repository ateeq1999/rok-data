//! `docs/pagination.md`'s samples, compiled.

use rok_data::prelude::*;

#[derive(Debug, Clone, Model, serde::Serialize)]
#[rok(per_page = 25, has_many(comments = Comment::POST_ID))]
struct Post {
    #[rok(generated)]
    id: i64,
    title: String,
    views: i32,
}

#[derive(Debug, Clone, Model, serde::Serialize)]
struct Comment {
    #[rok(generated)]
    id: i64,
    #[rok(belongs_to(post = Post))]
    post_id: i64,
}

async fn offset(db: Db, post: Post) -> rok_data::Result<()> {
    // a page and the total: one round trip
    let page = Post::query()
        .order_by(Post::ID)
        .paginate(&db, 2, 20)
        .await?;
    println!(
        "rows {:?}–{:?} of {}",
        page.first_item(),
        page.last_item(),
        page.total
    );

    // at the model's own size: `#[rok(per_page = 25)]`, 15 without it
    let page = Post::query().page(&db, 2).await?;

    // no total, no `COUNT(*)`: the rows and whether more follow
    let page = Post::query().simple_paginate(&db, 2, 20).await?;
    if page.has_more() {
        // link to page 3
    }

    // a relation's chain, and a page with its relations loaded
    let comments = post.comments().paginate(&db, 1, 50).await?;
    let page = Post::query()
        .with(Post::COMMENTS)
        .paginate(&db, 1, 20)
        .await?;
    Ok(())
}

async fn cursors(db: Db, token: Option<String>) -> rok_data::Result<()> {
    // a client's token parses back into a cursor (a bad one is
    // `Error::InvalidCursor`)
    let cursor: Option<Cursor> = token.map(|token| token.parse()).transpose()?;
    let page = Post::query()
        .order_by(Post::VIEWS.desc())
        .cursor_paginate(&db, cursor.as_ref(), 20)
        .await?;
    let next = page.next_cursor().map(ToString::to_string);
    let prev = page.prev_cursor().map(ToString::to_string);
    Ok(())
}

async fn walking(db: Db) -> rok_data::Result<()> {
    let db = &db;
    // windows by primary key: safe to delete what you were handed
    Post::filter(Post::VIEWS.eq(0))
        .chunk_by_id(db, 500, |posts| async move {
            for post in posts {
                post.delete(db).await?;
            }
            Ok(())
        })
        .await?;

    // a stream of rows, read a thousand at a time
    let mut posts = Post::query().lazy_by_id(db, 1000)?;
    while let Some(post) = posts.try_next().await? {
        // …
    }
    Ok(())
}

#[cfg(feature = "serde")]
fn shapes(page: &Page<Post>, uri: &str) -> serde_json::Result<serde_json::Value> {
    use rok_data::wire::Links;

    // the request's own URI: every link keeps its other query parameters
    let links = Links::from_request(uri);
    let paginator = serde_json::to_value(page.paginator(links.clone()))?;
    let resource = serde_json::to_value(page.resource(links))?;
    Ok(paginator)
}
