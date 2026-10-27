//! The chain's sugar: a condition over several columns, date parts, JSON
//! arrays and paths, column comparisons, `NULL`-aware equality, raw
//! escapes, `pipe` / `tap`, `UNION` and `increment_each`.

use super::*;

/// A table whose timestamp and JSON columns the struct leaves out: the
/// tests reach them through `Column::new`, so they need no `chrono` or
/// `json` feature.
#[derive(Debug, Clone, PartialEq, Model)]
struct Event {
    #[rok(generated)]
    id: i64,
    name: String,
    nickname: Option<String>,
}

const EVENTS: &str = r#"
CREATE TEMP TABLE events (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    nickname TEXT,
    happened_at TIMESTAMPTZ NOT NULL,
    data JSONB NOT NULL DEFAULT '{}'
);
"#;

const HAPPENED_AT: Column<Event> = Column::new("happened_at");
const DATA: Column<Event> = Column::new("data");

// ----- SQL shape -------------------------------------------------------------

#[test]
fn one_condition_over_several_columns() {
    let any = User::query()
        .filter_any([User::NAME, User::ROLE], |c| c.ilike("%an%"))
        .to_sql();
    assert_eq!(
        any.as_str(),
        r#"SELECT "id", "name", "role", "age" FROM "users" WHERE ("name" ILIKE $1 OR "role" ILIKE $2)"#
    );
    let all = User::query()
        .filter_all([User::NAME, User::ROLE], |c| c.is_not_null())
        .to_sql();
    assert!(
        all.as_str()
            .ends_with(r#"WHERE ("name" IS NOT NULL AND "role" IS NOT NULL)"#),
        "{all}"
    );
    let none = User::query()
        .filter_none([User::NAME, User::ROLE], |c| c.eq("x"))
        .to_sql();
    assert!(
        none.as_str()
            .ends_with(r#"WHERE NOT (("name" = $1 OR "role" = $2))"#),
        "{none}"
    );
}

#[test]
fn date_parts_json_and_column_comparisons() {
    let sql = Event::query()
        .filter(HAPPENED_AT.year().eq(2026))
        .filter(HAPPENED_AT.month().gte(10))
        .filter(DATA.json_array_len_at(["tags"]).gt(1))
        .filter(DATA.json_has_path(["owner", "id"]))
        .filter(Event::NICKNAME.ne_column(Event::NAME))
        .filter(Event::NICKNAME.is_not_distinct_from(None::<String>))
        .to_sql();
    assert_eq!(
        sql.as_str(),
        concat!(
            r#"SELECT "id", "name", "nickname" FROM "events" WHERE "#,
            r#"(CAST(EXTRACT(YEAR FROM "events"."happened_at") AS INTEGER)) = $1 AND "#,
            r#"(CAST(EXTRACT(MONTH FROM "events"."happened_at") AS INTEGER)) >= $2 AND "#,
            r#"(jsonb_array_length("events"."data" #> $3)) > $4 AND "#,
            r#"("events"."data" #> $5 IS NOT NULL) AND "#,
            r#""events"."nickname" <> "events"."name" AND "#,
            r#""nickname" IS NOT DISTINCT FROM $6"#,
        )
    );
    assert_eq!(
        Event::filter(HAPPENED_AT.date().eq("2026-10-27"))
            .to_sql()
            .as_str(),
        r#"SELECT "id", "name", "nickname" FROM "events" WHERE (CAST("events"."happened_at" AS DATE)) = $1"#
    );
}

#[test]
fn raw_escapes_bind_their_parameters_in_order() {
    let sql = Post::query()
        .filter_raw("lower(title) = ?", ["a1"])
        .group_by_raw("views / ?", [10])
        .having_raw("count(*) > ?", [1])
        .order_by_raw("views / ?", [10])
        .select_raw("views / ?", [10])
        .to_sql();
    assert_eq!(
        sql.as_str(),
        r#"SELECT (views / $1) FROM "posts" WHERE (lower(title) = $2) GROUP BY (views / $3) HAVING (count(*) > $4) ORDER BY (views / $5) ASC"#
    );
    assert_eq!(sql.params().len(), 5);
}

#[test]
fn pipe_and_tap_compose_the_chain() {
    fn popular(q: Select<Post>) -> Select<Post> {
        q.filter(Post::VIEWS.gte(100))
    }
    let mut seen = String::new();
    let sql = Post::query()
        .pipe(popular)
        .tap(|q| seen = q.to_sql().as_str().to_owned())
        .to_sql();
    assert_eq!(sql.as_str(), seen);
    assert!(sql.as_str().ends_with(r#"WHERE "views" >= $1"#), "{sql}");
    let count_sql = Post::query().pipe(|q| q.limit(1).to_sql());
    assert!(count_sql.as_str().ends_with("LIMIT 1"));
}

#[test]
fn a_union_renders_each_leg_and_numbers_their_parameters() {
    let sql = User::filter(User::ROLE.eq("admin"))
        .union(User::filter(User::AGE.gt(35)))
        .union_all(User::filter(User::NAME.eq("bob")).limit(1))
        .order_by(User::NAME)
        .limit(10)
        .to_sql();
    assert_eq!(
        sql.as_str(),
        concat!(
            r#"(SELECT "id", "name", "role", "age" FROM "users" WHERE "role" = $1)"#,
            r#" UNION (SELECT "id", "name", "role", "age" FROM "users" WHERE "age" > $2)"#,
            r#" UNION ALL (SELECT "id", "name", "role", "age" FROM "users" WHERE "name" = $3 LIMIT 1)"#,
            r#" ORDER BY "name" ASC LIMIT 10"#,
        )
    );
}

#[test]
fn increment_each_sets_every_column_in_one_update() {
    let sql = Visitor::query()
        .filter(Visitor::ID.eq(1))
        .update()
        .increment_each([(Visitor::VIEWS, 1), (Visitor::AGE, 2)])
        .to_sql();
    assert_eq!(
        sql.as_str(),
        r#"UPDATE "visitors" SET "views" = "views" + $1, "age" = "age" + $2 WHERE "id" = $3"#
    );
}

#[derive(Debug, Clone, PartialEq, Model)]
struct Visitor {
    #[rok(generated)]
    id: i64,
    name: String,
    age: i32,
    views: i32,
}

// ----- Database --------------------------------------------------------------

#[tokio::test]
async fn a_union_runs_and_orders_the_combined_rows() {
    let Some(db) = setup().await else { return };
    seed(&db).await;
    let names: Vec<String> = User::filter(User::ROLE.eq("admin"))
        .union(User::filter(User::AGE.gte(30)))
        .order_by(User::NAME.desc())
        .all(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|u| u.name)
        .collect();
    assert_eq!(names, ["cid", "ann"], "ann is in both legs, once");
    let twice = User::filter(User::ROLE.eq("admin"))
        .union_all(User::filter(User::AGE.gte(30)))
        .get(&db)
        .await
        .unwrap();
    assert_eq!(twice.len(), 3, "UNION ALL keeps ann twice");
}

#[tokio::test]
async fn the_column_conditions_match_the_rows_they_say() {
    let Some(db) = setup().await else { return };
    db.execute(EVENTS).await.unwrap();
    db.execute(
        r#"INSERT INTO events (name, nickname, happened_at, data) VALUES
        ('launch', 'launch', '2026-10-27 09:30:00+00', '{"tags": ["a", "b"], "owner": {"id": 1}}'),
        ('retro', NULL, '2025-03-01 17:00:00+00', '{"tags": ["a"]}'),
        ('party', 'bash', '2026-01-15 21:00:00+00', '{}')"#,
    )
    .await
    .unwrap();
    let names = |q: Select<Event>| {
        let db = db.clone();
        async move {
            let mut names: Vec<String> = q
                .all(&db)
                .await
                .unwrap()
                .into_iter()
                .map(|e| e.name)
                .collect();
            names.sort();
            names
        }
    };
    assert_eq!(
        names(Event::filter(HAPPENED_AT.year().eq(2026))).await,
        ["launch", "party"]
    );
    assert_eq!(
        names(Event::filter(HAPPENED_AT.month().eq(3))).await,
        ["retro"]
    );
    assert_eq!(
        names(Event::filter(HAPPENED_AT.hour().gte(17))).await,
        ["party", "retro"]
    );
    assert_eq!(
        names(Event::filter(DATA.json_array_len_at(["tags"]).gte(2))).await,
        ["launch"]
    );
    assert_eq!(
        names(Event::filter(DATA.json_has_path(["owner", "id"]))).await,
        ["launch"]
    );
    assert_eq!(
        names(Event::filter(Event::NICKNAME.eq_column(Event::NAME))).await,
        ["launch"]
    );
    assert_eq!(
        names(Event::filter(
            Event::NICKNAME.is_not_distinct_from(None::<String>)
        ))
        .await,
        ["retro"]
    );
    assert_eq!(
        names(Event::filter(Event::NICKNAME.is_distinct_from("bash"))).await,
        ["launch", "retro"],
        "NULL is distinct from \"bash\""
    );
    assert_eq!(
        names(Event::query().filter_any([Event::NAME, Event::NICKNAME], |c| c.eq("bash"))).await,
        ["party"]
    );
}

#[tokio::test]
async fn increment_each_adds_to_every_column() {
    let Some(db) = setup().await else { return };
    let visitor = Visitor::insert_builder()
        .set(Visitor::NAME, "vic")
        .set(Visitor::AGE, 20)
        .exec(&db)
        .await
        .unwrap();
    let after = Visitor::query()
        .filter(Visitor::ID.eq(visitor.id))
        .update()
        .increment_each([(Visitor::VIEWS, 3), (Visitor::AGE, 1)])
        .returning_one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((after.views, after.age), (3, 21));
}
