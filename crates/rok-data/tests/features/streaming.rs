//! Streaming and the aggregate projections.

use super::*;

#[tokio::test]
async fn streaming() {
    let Some(db) = setup().await else { return };
    seed(&db).await;

    let mut stream = Post::order_by(Post::VIEWS.desc()).stream(&db);
    let mut seen = Vec::new();
    while let Some(post) = stream.try_next().await.unwrap() {
        seen.push(post.views);
    }
    assert_eq!(seen, [100, 30, 20, 15, 10, 5]);
    drop(stream);

    let titles: Vec<(String,)> = Post::filter(Post::VIEWS.lt(15))
        .order_by(Post::TITLE)
        .select(Post::TITLE)
        .stream(&db)
        .try_collect()
        .await
        .unwrap();
    assert_eq!(titles, [("a1".to_string(),), ("b1".to_string(),)]);

    let n = raw("SELECT * FROM users")
        .stream::<User, _>(&db)
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(n.len(), 3);

    // Streams work inside transactions too.
    let mut tx = db.begin().await.unwrap();
    let count = User::query()
        .stream(&mut *tx)
        .try_collect::<Vec<_>>()
        .await
        .unwrap()
        .len();
    assert_eq!(count, 3);
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn aggregates_and_projections() {
    let Some(db) = setup().await else { return };
    seed(&db).await;

    assert_eq!(
        Post::query().sum(&db, Post::VIEWS).await.unwrap(),
        Some(180)
    );
    assert_eq!(
        Post::filter(Post::VIEWS.gt(1000))
            .sum(&db, Post::VIEWS)
            .await
            .unwrap(),
        None
    );
    assert_eq!(User::query().avg(&db, User::AGE).await.unwrap(), Some(30.0));
    assert_eq!(
        User::query().min::<i32, _>(&db, User::AGE).await.unwrap(),
        Some(20)
    );
    assert_eq!(
        User::query()
            .max::<String, _>(&db, User::NAME)
            .await
            .unwrap(),
        Some("cid".into())
    );

    let per_author: Vec<(i64, i64, i64)> = Post::query()
        .group_by(Post::AUTHOR_ID)
        .having(Projection::count_all().gte(2))
        .order_by(Post::AUTHOR_ID)
        .select((
            Post::AUTHOR_ID,
            Projection::count_all(),
            Post::VIEWS.sum().cast("BIGINT"),
        ))
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(per_author.len(), 2);
    assert_eq!((per_author[0].1, per_author[0].2), (3, 60));
    assert_eq!((per_author[1].1, per_author[1].2), (2, 20));

    #[derive(rok_data::FromRow)]
    struct RoleStats {
        role: String,
        users: i64,
        avg_age: f64,
    }
    let stats: Vec<RoleStats> = User::query()
        .group_by(User::ROLE)
        .order_by(User::ROLE)
        .select((
            User::ROLE,
            Projection::count_all().alias("users"),
            User::AGE.avg().cast("DOUBLE PRECISION").alias("avg_age"),
        ))
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(stats.len(), 2);
    assert_eq!(
        (stats[1].role.as_str(), stats[1].users, stats[1].avg_age),
        ("member", 2, 30.0)
    );

    let names: i64 = User::query()
        .select(User::NAME.count_distinct())
        .scalar(&db)
        .await
        .unwrap();
    assert_eq!(names, 3);
    let (name, age): (String, i32) = User::order_by(User::AGE.desc())
        .select((User::NAME, User::AGE))
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!((name.as_str(), age), ("cid", 40));
}
