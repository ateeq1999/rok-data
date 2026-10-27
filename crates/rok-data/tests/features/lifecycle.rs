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

#[derive(Debug, Clone, PartialEq, Model)]
#[rok(table = "archived_tags")]
struct ArchivedTag {
    #[rok(generated)]
    id: i64,
    slug: String,
}

#[test]
fn insert_into_writes_a_select_into_another_table() {
    let sql = Tag::filter(Tag::USES.lt(2))
        .select((Tag::SLUG,))
        .insert_into::<ArchivedTag>([ArchivedTag::SLUG])
        .to_sql();
    assert_eq!(
        sql.as_str(),
        r#"INSERT INTO "archived_tags" ("slug") SELECT "slug" FROM "tags" WHERE "uses" < $1"#
    );
}

#[tokio::test]
async fn insert_into_copies_the_rows_in_the_database() {
    let Some(db) = setup().await else { return };
    db.execute(TAGS).await.unwrap();
    db.execute("CREATE TEMP TABLE archived_tags (id BIGSERIAL PRIMARY KEY, slug TEXT NOT NULL)")
        .await
        .unwrap();
    for (slug, uses) in [("a", 1), ("b", 5), ("c", 0)] {
        tag(slug, uses).insert(&db).await.unwrap();
    }
    let rarely_used = || {
        Tag::filter(Tag::USES.lt(2))
            .order_by(Tag::SLUG)
            .select((Tag::SLUG,))
    };
    let archived = rarely_used()
        .insert_into::<ArchivedTag>([ArchivedTag::SLUG])
        .returning(&db)
        .await
        .unwrap();
    let slugs: Vec<&str> = archived.iter().map(|t| t.slug.as_str()).collect();
    assert_eq!(slugs, ["a", "c"]);
    // A column count that doesn't match the selection is refused.
    let error = rarely_used()
        .insert_into::<ArchivedTag>([ArchivedTag::SLUG, ArchivedTag::ID])
        .exec(&db)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("2 column(s) for 1"), "{error}");
}
