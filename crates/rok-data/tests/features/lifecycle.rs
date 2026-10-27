//! A row's lifecycle verbs: `first_or_insert`, `increment` / `decrement`,
//! `touch`.

use super::*;

#[derive(Debug, Clone, PartialEq, Model)]
struct Tag {
    #[rok(generated)]
    id: i64,
    slug: String,
    uses: i32,
}

const TAGS: &str = "CREATE TEMP TABLE tags (id BIGSERIAL PRIMARY KEY, slug TEXT NOT NULL UNIQUE, uses INT NOT NULL DEFAULT 0)";

fn tag(slug: &str, uses: i32) -> Tag {
    Tag {
        id: 0,
        slug: slug.into(),
        uses,
    }
}

#[tokio::test]
async fn first_or_insert_inserts_once_then_finds() {
    let Some(db) = setup().await else { return };
    db.execute(TAGS).await.unwrap();
    let first = tag("rust", 1)
        .first_or_insert(&db, [Tag::SLUG])
        .await
        .unwrap();
    assert_eq!((first.slug.as_str(), first.uses), ("rust", 1));
    // The same slug again: the stored row, not this one's `uses`.
    let again = tag("rust", 99)
        .first_or_insert(&db, [Tag::SLUG])
        .await
        .unwrap();
    assert_eq!(again, first);
    assert_eq!(Tag::query().count(&db).await.unwrap(), 1);
    // No column to match on is a mistake, not a plain insert.
    let error = tag("sql", 1)
        .first_or_insert(&db, Vec::<Column<Tag>>::new())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("names no column"), "{error}");
}

#[tokio::test]
async fn increment_and_decrement_write_in_the_database() {
    let Some(db) = setup().await else { return };
    db.execute(TAGS).await.unwrap();
    let stored = tag("go", 5).insert(&db).await.unwrap();
    // The in-memory value is stale on purpose: the update doesn't read it.
    let stale = Tag { uses: 0, ..stored };
    assert_eq!(stale.increment(&db, Tag::USES, 3).await.unwrap().uses, 8);
    assert_eq!(stale.decrement(&db, Tag::USES, 2).await.unwrap().uses, 6);
    // `touch` needs an `updated_at` column.
    let error = stale.touch(&db).await.unwrap_err();
    assert!(error.to_string().contains("no `updated_at`"), "{error}");
}
