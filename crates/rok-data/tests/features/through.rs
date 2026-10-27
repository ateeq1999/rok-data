//! Relations through an intermediate model: a country has many notes
//! through its citizens, and one capital mayor through its capital.

use super::*;

#[derive(Debug, Clone, PartialEq, Model)]
#[rok(
    table = "countries",
    has_many_through(notes = Note::CITIZEN_ID, through = Citizen::COUNTRY_ID),
    has_one_through(mayor = Mayor::CITY_ID, through = City::COUNTRY_ID)
)]
struct Country {
    #[rok(generated)]
    id: i64,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Model)]
struct Citizen {
    #[rok(generated)]
    id: i64,
    country_id: i64,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Model)]
struct Note {
    #[rok(generated)]
    id: i64,
    citizen_id: i64,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Model)]
#[rok(table = "cities")]
struct City {
    #[rok(generated)]
    id: i64,
    country_id: i64,
}

#[derive(Debug, Clone, PartialEq, Model)]
struct Mayor {
    #[rok(generated)]
    id: i64,
    city_id: i64,
    name: String,
}

const TABLES: &str = r#"
CREATE TEMP TABLE countries (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL);
CREATE TEMP TABLE citizens (
    id BIGSERIAL PRIMARY KEY, country_id BIGINT NOT NULL REFERENCES countries (id), name TEXT NOT NULL
);
CREATE TEMP TABLE notes (
    id BIGSERIAL PRIMARY KEY, citizen_id BIGINT NOT NULL REFERENCES citizens (id), body TEXT NOT NULL
);
CREATE TEMP TABLE cities (id BIGSERIAL PRIMARY KEY, country_id BIGINT NOT NULL REFERENCES countries (id));
CREATE TEMP TABLE mayors (
    id BIGSERIAL PRIMARY KEY, city_id BIGINT NOT NULL REFERENCES cities (id), name TEXT NOT NULL
);
"#;

// ----- SQL shape -------------------------------------------------------------

#[test]
fn a_through_query_reaches_the_target_via_the_intermediate_model() {
    let country = Country {
        id: 7,
        name: "x".into(),
    };
    assert_eq!(
        country.notes().to_sql().as_str(),
        concat!(
            r#"SELECT "id", "citizen_id", "body" FROM "notes" WHERE "citizen_id" IN ("#,
            r#"SELECT "id" FROM "citizens" WHERE "country_id" = $1)"#,
        )
    );
    assert!(
        Country::query()
            .has(Country::NOTES)
            .to_sql()
            .as_str()
            .ends_with(concat!(
                r#"WHERE EXISTS (SELECT 1 FROM "notes" WHERE "citizen_id" IN ("#,
                r#"SELECT "id" FROM "citizens" WHERE "citizens"."country_id" = "countries"."id"))"#,
            ))
    );
}

// ----- Database --------------------------------------------------------------

/// Two countries: `aa` with citizens ann (2 notes) and al (1 note) and a
/// mayor; `bb` with citizen bo and no notes or mayor.
async fn seeded() -> Option<(Db, Country, Country)> {
    let db = setup().await?;
    db.execute(TABLES).await.unwrap();
    let country = |name: &str| {
        Country::insert_builder()
            .set(Country::NAME, name.to_owned())
            .exec(&db)
    };
    let aa = country("aa").await.unwrap();
    let bb = country("bb").await.unwrap();
    let citizen = |country: i64, name: &str| {
        Citizen::insert_builder()
            .set(Citizen::COUNTRY_ID, country)
            .set(Citizen::NAME, name.to_owned())
            .exec(&db)
    };
    let ann = citizen(aa.id, "ann").await.unwrap();
    let al = citizen(aa.id, "al").await.unwrap();
    citizen(bb.id, "bo").await.unwrap();
    for (citizen, body) in [(&ann, "n1"), (&ann, "n2"), (&al, "n3")] {
        Note::insert_builder()
            .set(Note::CITIZEN_ID, citizen.id)
            .set(Note::BODY, body)
            .exec(&db)
            .await
            .unwrap();
    }
    let city = City::insert_builder()
        .set(City::COUNTRY_ID, aa.id)
        .exec(&db)
        .await
        .unwrap();
    Mayor::insert_builder()
        .set(Mayor::CITY_ID, city.id)
        .set(Mayor::NAME, "mo")
        .exec(&db)
        .await
        .unwrap();
    Some((db, aa, bb))
}

#[tokio::test]
async fn through_relations_load_query_filter_and_count() {
    let Some((db, aa, bb)) = seeded().await else {
        return;
    };
    let mut bodies: Vec<String> = aa
        .notes()
        .all(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|n| n.body)
        .collect();
    bodies.sort();
    assert_eq!(bodies, ["n1", "n2", "n3"]);

    let view = Country::query()
        .order_by(Country::ID)
        .with(Country::NOTES)
        .with(Country::MAYOR)
        .with_count(Country::NOTES)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(
        view.try_get(Country::NOTES, &aa).map(<[Note]>::len),
        Some(3)
    );
    assert_eq!(
        view.try_get(Country::NOTES, &bb).map(<[Note]>::len),
        Some(0)
    );
    assert_eq!(
        view.try_get(Country::MAYOR, &aa)
            .flatten()
            .map(|m| m.name.as_str()),
        Some("mo")
    );
    assert_eq!(view.try_get(Country::MAYOR, &bb), Some(None));
    assert_eq!(view.count(Country::NOTES, &aa), Some(3));
    assert_eq!(view.count(Country::NOTES, &bb), Some(0));

    let with_notes: Vec<String> = Country::query()
        .has(Country::NOTES)
        .all(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(with_notes, ["aa"]);

    let loaded = Country::NOTES
        .load(&db, &[aa.clone(), bb.clone()])
        .await
        .unwrap();
    assert_eq!((loaded.get(&aa).len(), loaded.get(&bb).len()), (3, 0));
}
