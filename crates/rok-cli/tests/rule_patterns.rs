//! The `mobile` and `postal_code` pattern tables live twice — in the
//! runtime (`rok_data::validate::patterns`, for `#[derive(Input)]`) and in
//! the engine (generated into an API's checks). They must stay equal.

#[test]
fn the_runtime_and_the_engine_carry_the_same_patterns() {
    assert_eq!(
        rok_data::validate::patterns::MOBILE_PATTERNS,
        rok_data_sqlc::rule_patterns::MOBILE_PATTERNS
    );
    assert_eq!(
        rok_data::validate::patterns::POSTAL_PATTERNS,
        rok_data_sqlc::rule_patterns::POSTAL_PATTERNS
    );
}
