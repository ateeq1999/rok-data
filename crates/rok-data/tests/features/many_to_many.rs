//! Many-to-many through a pivot table: members belong to many groups.

use super::*;

#[derive(Debug, Clone, PartialEq, Model)]
#[rok(belongs_to_many(groups = Group, pivot = "group_member"))]
struct Member {
    #[rok(generated)]
    id: i64,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Model)]
#[rok(belongs_to_many(
    members = Member,
    pivot = "group_member",
    foreign_key = "group_id",
    related_key = "member_id"
))]
struct Group {
    #[rok(generated)]
    id: i64,
    label: String,
}

const TABLES: &str = r#"
CREATE TEMP TABLE members (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL);
CREATE TEMP TABLE groups (id BIGSERIAL PRIMARY KEY, label TEXT NOT NULL);
CREATE TEMP TABLE group_member (
    member_id BIGINT NOT NULL REFERENCES members (id),
    group_id BIGINT NOT NULL REFERENCES groups (id),
    role TEXT NOT NULL DEFAULT 'member',
    UNIQUE (member_id, group_id)
);
"#;

// ----- SQL shape -------------------------------------------------------------

#[test]
fn the_pivot_columns_default_from_the_model_names() {
    assert_eq!(Member::GROUPS.pivot(), "group_member");
    let member = Member {
        id: 7,
        name: "x".into(),
    };
    assert_eq!(
        member.groups().to_sql().as_str(),
        concat!(
            r#"SELECT "id", "label" FROM "groups" WHERE ("groups"."id" IN ("#,
            r#"SELECT "group_member"."group_id" FROM "group_member" WHERE "group_member"."member_id" = $1))"#,
        )
    );
}

// ----- Database --------------------------------------------------------------

async fn names(db: &Db, member: &Member) -> Vec<String> {
    let mut labels: Vec<String> = member
        .groups()
        .all(db)
        .await
        .unwrap()
        .into_iter()
        .map(|g| g.label)
        .collect();
    labels.sort();
    labels
}

#[tokio::test]
async fn the_pivot_is_written_and_read_from_both_sides() {
    let Some(db) = setup().await else { return };
    db.execute(TABLES).await.unwrap();
    let member = |name: &str| {
        Member::insert_builder()
            .set(Member::NAME, name.to_owned())
            .exec(&db)
    };
    let group = |label: &str| {
        Group::insert_builder()
            .set(Group::LABEL, label.to_owned())
            .exec(&db)
    };
    let (ann, bob) = (member("ann").await.unwrap(), member("bob").await.unwrap());
    let (red, blue, green) = (
        group("red").await.unwrap(),
        group("blue").await.unwrap(),
        group("green").await.unwrap(),
    );

    // attach skips what is there already.
    assert_eq!(
        Member::GROUPS
            .attach(&db, &ann, [red.id, blue.id])
            .await
            .unwrap(),
        2
    );
    assert_eq!(Member::GROUPS.attach(&db, &ann, [red.id]).await.unwrap(), 0);
    assert_eq!(names(&db, &ann).await, ["blue", "red"]);

    // sync makes the set exact; toggle flips each.
    Member::GROUPS
        .sync(&db, &ann, [blue.id, green.id])
        .await
        .unwrap();
    assert_eq!(names(&db, &ann).await, ["blue", "green"]);
    Member::GROUPS
        .toggle(&db, &ann, [blue.id, red.id])
        .await
        .unwrap();
    assert_eq!(names(&db, &ann).await, ["green", "red"]);
    assert_eq!(
        Member::GROUPS.detach(&db, &ann, [green.id]).await.unwrap(),
        1
    );
    assert_eq!(names(&db, &ann).await, ["red"]);

    // Pivot columns, written and updated.
    Member::GROUPS
        .attach_with(&db, &bob, [red.id], [("role", "owner".into())])
        .await
        .unwrap();
    assert_eq!(
        Member::GROUPS
            .update_existing_pivot(&db, &ann, red.id, [("role", "admin".into())])
            .await
            .unwrap(),
        1
    );
    let roles: Vec<(String,)> = rok_data::raw("SELECT role FROM group_member ORDER BY role")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(roles, [("admin".to_owned(),), ("owner".to_owned(),)]);

    // The other side, loaded, filtered and counted.
    let view = Group::query()
        .order_by(Group::LABEL)
        .with(Group::MEMBERS)
        .with_count(Group::MEMBERS)
        .all(&db)
        .await
        .unwrap();
    let mut on_red: Vec<&str> = view
        .try_get(Group::MEMBERS, &red)
        .unwrap()
        .iter()
        .map(|m| m.name.as_str())
        .collect();
    on_red.sort_unstable();
    assert_eq!(on_red, ["ann", "bob"]);
    assert_eq!(view.count(Group::MEMBERS, &red), Some(2));
    assert_eq!(view.count(Group::MEMBERS, &blue), Some(0));
    let used: Vec<String> = Group::query()
        .has(Group::MEMBERS)
        .all(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|g| g.label)
        .collect();
    assert_eq!(used, ["red"]);

    // An empty sync detaches everything.
    Member::GROUPS
        .sync(&db, &ann, Vec::<i64>::new())
        .await
        .unwrap();
    assert!(names(&db, &ann).await.is_empty());
}
