//! The cached queries.

use super::*;

#[tokio::test]
async fn memoized_queries() {
    let Some(db) = setup_with(Db::builder().query_cache(100)).await else {
        return;
    };
    seed(&db).await;
    let cache = db.cache().expect("cache enabled").clone();
    let ttl = Duration::from_secs(60);
    let admins = || User::filter(User::ROLE.eq("admin")).memoize(ttl);

    assert_eq!(admins().all(&db).await.unwrap().len(), 1);
    assert_eq!(admins().all(&db).await.unwrap().len(), 1);
    assert_eq!(admins().count(&db).await.unwrap(), 1);
    assert_eq!(admins().count(&db).await.unwrap(), 1);
    let stats = cache.stats();
    assert_eq!((stats.hits, stats.misses), (2, 2));

    // Different parameters are different entries.
    assert_eq!(
        User::filter(User::ROLE.eq("member"))
            .memoize(ttl)
            .count(&db)
            .await
            .unwrap(),
        2
    );
    assert_eq!(cache.stats().misses, 3);

    // A write through rok-data invalidates the table.
    User::insert_builder()
        .set(User::NAME, "dee")
        .set(User::ROLE, "admin")
        .set(User::AGE, 50)
        .exec(&db)
        .await
        .unwrap();
    assert_eq!(admins().count(&db).await.unwrap(), 2);
    assert_eq!(admins().all(&db).await.unwrap().len(), 2);

    // ...but only that table.
    let posts = || Post::query().memoize(ttl);
    assert_eq!(posts().count(&db).await.unwrap(), 6);
    User::filter(User::NAME.eq("dee"))
        .delete(&db)
        .await
        .unwrap();
    let before = cache.stats().hits;
    assert_eq!(posts().count(&db).await.unwrap(), 6);
    assert_eq!(cache.stats().hits, before + 1);

    // Writes in a transaction invalidate (again on commit).
    admins().all(&db).await.unwrap();
    let mut tx = db.begin().await.unwrap();
    User::filter(User::ROLE.eq("admin"))
        .update()
        .set(User::ROLE, "owner")
        .exec(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(admins().count(&db).await.unwrap(), 0);

    // Raw writes are invisible unless declared.
    let owners = || User::filter(User::ROLE.eq("owner")).memoize(ttl);
    assert_eq!(owners().count(&db).await.unwrap(), 1);
    raw("UPDATE users SET role = 'admin' WHERE role = 'owner'")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        owners().count(&db).await.unwrap(),
        1,
        "stale until invalidated"
    );
    raw("UPDATE users SET role = 'admin' WHERE role = 'owner'")
        .invalidates("users")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(owners().count(&db).await.unwrap(), 0);
    cache.clear();
    assert_eq!(cache.stats().entries, 0);

    // Pagination and first/exists are memoized too.
    let page = User::order_by(User::ID)
        .memoize(ttl)
        .paginate(&db, 1, 2)
        .await
        .unwrap();
    let again = User::order_by(User::ID)
        .memoize(ttl)
        .paginate(&db, 1, 2)
        .await
        .unwrap();
    assert_eq!(page, again);
    assert!(User::query().memoize(ttl).exists(&db).await.unwrap());
    assert!(
        User::order_by(User::ID)
            .memoize(ttl)
            .first(&db)
            .await
            .unwrap()
            .is_some()
    );

    // Without a cache, memoize is a no-op.
    let before = cache.stats();
    let plain = db.pool();
    assert_eq!(admins().count(plain).await.unwrap(), 1);
    assert_eq!(admins().count(plain).await.unwrap(), 1);
    let after = cache.stats();
    assert_eq!((after.hits, after.misses), (before.hits, before.misses));
}

#[tokio::test]
async fn memoize_expires() {
    let Some(db) = setup_with(Db::builder().query_cache(10)).await else {
        return;
    };
    seed(&db).await;
    let q = || User::query().memoize(Duration::from_millis(50));
    q().count(&db).await.unwrap();
    tokio::time::sleep(Duration::from_millis(80)).await;
    q().count(&db).await.unwrap();
    assert_eq!(db.cache().unwrap().stats().hits, 0);
}
