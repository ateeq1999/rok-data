//! Validation rules on the create inputs and update patches generated with
//! `derive = ["serde", "validate"]` (`src/validated_db/`, from
//! `validated_schema.sql`). No database needed.

use rok_data::{Input, ValidationErrors, Value};
use rok_data_sqlc_fixture_app::validated_db::players::models::{NewPlayer, UpdatePlayer};
use rok_data_sqlc_fixture_app::validated_db::seasons::models::{NewSeason, SeasonId, UpdateSeason};
use rok_data_sqlc_fixture_app::validated_db::teams::models::{NewTeam, TeamId, UpdateTeam};
use rust_decimal::Decimal;

fn player() -> NewPlayer {
    NewPlayer {
        team_code: Some(TeamId("NORTH".to_owned())),
        handle: "ace".to_owned(),
        bio: None,
        jersey: 7,
        rating: Some(9.5),
        salary: Some(Decimal::new(125_000, 2)),
        nickname: None,
        joined_year: None,
    }
}

/// The input's field rules: `Ok` when it passes them, otherwise its errors.
fn checked<T: Input<Context = ()>>(input: T) -> Result<(), ValidationErrors> {
    match input.validate_fields() {
        Ok(_) => Ok(()),
        Err(rok_data::Error::Validation(errors)) => Err(errors),
        Err(other) => panic!("not a validation error: {other}"),
    }
}

/// Each failing field with its error's code and message.
fn failures(errors: &ValidationErrors) -> Vec<(String, String, String)> {
    let mut failures: Vec<(String, String, String)> = errors
        .errors()
        .iter()
        .map(|error| {
            (
                error.field.clone(),
                error.code.to_owned(),
                error.message.clone(),
            )
        })
        .collect();
    failures.sort();
    failures
}

/// The `params` of `field`'s first error.
fn params(errors: &ValidationErrors, field: &str) -> Vec<(&'static str, Value)> {
    let error = errors.errors().iter().find(|error| error.field == field);
    error.expect("an error on the field").params.clone()
}

fn failure(field: &str, code: &str, message: &str) -> (String, String, String) {
    (field.to_owned(), code.to_owned(), message.to_owned())
}

#[test]
fn a_valid_input_passes() {
    assert_eq!(checked(player()), Ok(()));
    // Lengths count characters, not bytes, as PostgreSQL counts them.
    let wide = NewPlayer {
        handle: "é".repeat(20),
        nickname: Some(Some("ü".repeat(12))),
        ..player()
    };
    assert_eq!(checked(wide), Ok(()));
}

#[test]
fn an_input_breaking_rules_fails_field_by_field() {
    let input = NewPlayer {
        team_code: Some(TeamId("NORTHWEST".to_owned())),
        handle: "x".repeat(21),
        bio: Some("y".repeat(201)),
        jersey: 100,
        rating: Some(0.0),
        salary: Some(Decimal::new(-1, 2)),
        nickname: Some(Some("z".repeat(13))),
        // Only the database checks this one.
        joined_year: Some(1800),
    };
    let errors = checked(input).expect_err("every rule is broken");
    assert_eq!(
        failures(&errors),
        [
            failure("bio", "length", "bio must be at most 200 characters"),
            failure("handle", "length", "handle must be at most 20 characters"),
            failure("jersey", "range", "jersey must be between 0 and 99"),
            failure(
                "nickname",
                "length",
                "nickname must be at most 12 characters"
            ),
            failure(
                "rating",
                "range",
                "rating must be greater than 0 and at most 10"
            ),
            failure("salary", "range", "salary must be at least 0"),
            failure(
                "team_code",
                "length",
                "team_code must be at most 8 characters"
            ),
        ]
    );
}

/// A `NUMERIC` column takes `range` like any number, its error carrying
/// the bounds as `params` (an integer bound as an integer).
#[test]
fn a_decimal_range_error_carries_its_bounds() {
    let input = NewPlayer {
        salary: Some(Decimal::new(-1, 2)),
        ..player()
    };
    let errors = checked(input).expect_err("salary is negative");
    assert_eq!(params(&errors, "salary"), [("min", Value::from(0_i64))]);
    // The same names as an integer column's `range`.
    let input = NewPlayer {
        jersey: 100,
        ..player()
    };
    let errors = checked(input).expect_err("jersey is out of range");
    assert_eq!(
        params(&errors, "jersey"),
        [("min", Value::from(0_i64)), ("max", Value::from(99_i64))]
    );
}

#[test]
fn absent_and_null_input_fields_pass() {
    let input = NewPlayer {
        team_code: None,
        rating: None,
        salary: None,
        nickname: Some(None),
        ..player()
    };
    assert_eq!(checked(input), Ok(()));
}

#[test]
fn a_typed_text_key_keeps_its_length_rule() {
    let team = NewTeam {
        code: TeamId("NORTHWEST".to_owned()),
        name: "N".to_owned(),
    };
    let errors = checked(team).expect_err("both rules are broken");
    assert_eq!(
        failures(&errors),
        [
            failure("code", "length", "code must be at most 8 characters"),
            failure("name", "length", "name must be between 2 and 40 characters"),
        ]
    );
}

#[test]
fn a_patch_checks_only_the_values_it_sets() {
    assert_eq!(checked(UpdatePlayer::default()), Ok(()));
    assert_eq!(checked(UpdateTeam::default()), Ok(()));
    // `Some(None)` sets NULL, which no rule constrains.
    let nulls = UpdatePlayer {
        bio: Some(None),
        rating: Some(None),
        nickname: Some(None),
        team_code: Some(None),
        ..UpdatePlayer::default()
    };
    assert_eq!(checked(nulls), Ok(()));

    let patch = UpdatePlayer {
        handle: Some("ok".to_owned()),
        bio: Some(Some("y".repeat(201))),
        jersey: Some(-1),
        salary: Some(Decimal::new(1, 2)),
        ..UpdatePlayer::default()
    };
    let errors = checked(patch).expect_err("two rules are broken");
    assert_eq!(
        failures(&errors),
        [
            failure("bio", "length", "bio must be at most 200 characters"),
            failure("jersey", "range", "jersey must be between 0 and 99"),
        ]
    );

    let team = UpdateTeam {
        name: Some("Northern".to_owned()),
    };
    assert_eq!(checked(team), Ok(()));
}

#[test]
fn a_typed_integer_key_keeps_its_range_rule() {
    let season = NewSeason {
        year: SeasonId(2024),
        previous_year: Some(SeasonId(2023)),
        title: "Spring".to_owned(),
    };
    assert_eq!(checked(season), Ok(()));

    let season = NewSeason {
        year: SeasonId(1850),
        previous_year: Some(SeasonId(1849)),
        title: "Spring".to_owned(),
    };
    let errors = checked(season).expect_err("both ranges are broken");
    assert_eq!(
        failures(&errors),
        [
            failure(
                "previous_year",
                "range",
                "previous_year must be at least 1900"
            ),
            failure("year", "range", "year must be between 1900 and 2100"),
        ]
    );
    // The bounds, without the rejected value.
    assert_eq!(
        params(&errors, "year"),
        [
            ("min", Value::from(1900_i64)),
            ("max", Value::from(2100_i64))
        ]
    );

    // A patch checks the id only when it sets one.
    assert_eq!(checked(UpdateSeason::default()), Ok(()));
    let patch = UpdateSeason {
        previous_year: Some(Some(SeasonId(1899))),
        ..UpdateSeason::default()
    };
    assert_eq!(
        failures(&checked(patch).expect_err("the range is broken")),
        [failure(
            "previous_year",
            "range",
            "previous_year must be at least 1900"
        )]
    );
}
