//! Walking a large table: `chunk`, `chunk_by_id`, the lazy streams, and
//! `each` / `each_by_id`, with early stops.

use super::*;

/// Archives one post: a write through the same pool the walk reads from —
/// the documented callback shape.
async fn archive(db: &Db, post: Post) -> rok_data::Result<()> {
    Post::filter(Post::ID.eq(post.id))
        .update()
        .set(Post::VIEWS, 0)
        .exec(db)
        .await?;
    Ok(())
}

#[tokio::test]
async fn the_documented_chunk_callback_compiles_and_writes_back() {
    let Some(db) = setup().await else { return };
    seed(&db).await;
    // The documented shape: a reference the `async move` blocks copy.
    let db = &db;
    Post::query()
        .chunk(db, 4, |rows| async move {
            for post in rows {
                archive(db, post).await?;
            }
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(Post::filter(Post::VIEWS.gt(0)).count(db).await.unwrap(), 0);
}

#[tokio::test]
async fn chunk_by_id_visits_every_row_once_while_the_callback_deletes_them() {
    let Some(db) = setup().await else { return };
    seed(&db).await;
    let db = &db;
    let seen = std::sync::Mutex::new(Vec::new());
    let seen = &seen;
    Post::query()
        .chunk_by_id(db, 2, |rows| async move {
            for post in rows {
                seen.lock().unwrap().push(post.id);
                // Deleting what was just read: an offset walk would now skip
                // rows; the keyset walk starts after the last key instead.
                post.delete(db).await?;
            }
            Ok(())
        })
        .await
        .unwrap();
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 6, "every post exactly once: {seen:?}");
    let mut sorted = seen.clone();
    sorted.sort_unstable();
    assert_eq!(seen, sorted, "by primary key");
    assert_eq!(Post::query().count(db).await.unwrap(), 0);
}

#[tokio::test]
async fn a_walk_stops_when_the_callback_says_so() {
    let Some(db) = setup().await else { return };
    seed(&db).await;
    let windows = std::sync::atomic::AtomicUsize::new(0);
    let windows = &windows;
    Post::query()
        .chunk_by_id(&db, 2, |_rows| async move {
            windows.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(false) // stop after the first window
        })
        .await
        .unwrap();
    assert_eq!(windows.load(std::sync::atomic::Ordering::SeqCst), 1);

    let mut rows = 0;
    Post::query()
        .order_by(Post::ID)
        .each(&db, |_post| {
            rows += 1;
            let more = rows < 3;
            async move { Ok(more) }
        })
        .await
        .unwrap();
    assert_eq!(rows, 3, "`each` stopped at the third row");
}

#[tokio::test]
async fn the_lazy_streams_read_in_windows_in_order() {
    let Some(db) = setup().await else { return };
    let (_, posts) = seed(&db).await;
    let mut ids: Vec<i64> = posts.iter().map(|post| post.id).collect();
    ids.sort_unstable();

    let by_id: Vec<i64> = Post::query()
        .lazy_by_id(&db, 4)
        .unwrap()
        .map_ok(|post| post.id)
        .try_collect()
        .await
        .unwrap();
    assert_eq!(by_id, ids);

    let desc: Vec<i64> = Post::query()
        .lazy_by_id_desc(&db, 4)
        .unwrap()
        .map_ok(|post| post.id)
        .try_collect()
        .await
        .unwrap();
    assert_eq!(desc, ids.iter().rev().copied().collect::<Vec<_>>());

    let ordered: Vec<i64> = Post::query()
        .order_by(Post::ID.desc())
        .lazy(&db, 4)
        .unwrap()
        .map_ok(|post| post.id)
        .try_collect()
        .await
        .unwrap();
    assert_eq!(ordered, desc, "`lazy` keeps the query's own order");

    let mut each = Vec::new();
    Post::query()
        .each_by_id(&db, |post| {
            each.push(post.id);
            async { Ok(()) }
        })
        .await
        .unwrap();
    assert_eq!(each, ids);
}

#[tokio::test]
async fn a_zero_window_is_an_error() {
    let pool = rok_data::sqlx::PgPool::connect_lazy("postgres://localhost/none").unwrap();
    assert!(Post::query().lazy(&pool, 0).is_err());
    assert!(Post::query().lazy_by_id(&pool, 0).is_err());
}
