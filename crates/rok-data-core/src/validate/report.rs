//! Reporting validation errors: an [`ErrorReporter`] turns them into a
//! response body; [`DetailsReporter`] writes the generated APIs' `422
//! validation_failed` body, so hand-written and generated endpoints answer
//! alike.

use serde_json::{Map, Value as Json, json};

use crate::Value;

use super::ValidationErrors;

/// Turns [`ValidationErrors`] into what an endpoint answers.
pub trait ErrorReporter {
    /// The body (or whole response) the reporter produces.
    type Output;

    /// The report of `errors`.
    fn report(&self, errors: &ValidationErrors) -> Self::Output;
}

/// The wire standard's validation body:
///
/// ```json
/// {"error": {"code": "validation_failed",
///            "message": "the request body failed validation",
///            "details": {"email": [{"code": "email", "message": "…", "params": {}}]}}}
/// ```
///
/// `details` holds each failing path's errors in order; record-level
/// errors sit under `""`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DetailsReporter;

impl ErrorReporter for DetailsReporter {
    type Output = Json;

    fn report(&self, errors: &ValidationErrors) -> Json {
        let mut details = Map::new();
        for error in errors.errors() {
            let params: Map<String, Json> = error
                .params
                .iter()
                .map(|(name, value)| ((*name).to_owned(), json_of(value)))
                .collect();
            let entry = json!({
                "code": error.code,
                "message": error.message,
                "params": params,
            });
            match details
                .entry(error.field.clone())
                .or_insert_with(|| Json::Array(Vec::new()))
            {
                Json::Array(list) => list.push(entry),
                other => *other = Json::Array(vec![entry]),
            }
        }
        json!({
            "error": {
                "code": "validation_failed",
                "message": "the request body failed validation",
                "details": details,
            }
        })
    }
}

/// A parameter as JSON: numbers as numbers, text as text, `NULL` as null,
/// anything else as its display.
pub(crate) fn json_of(value: &Value) -> Json {
    match value {
        Value::Bool(Some(b)) => json!(b),
        Value::I16(Some(n)) => json!(n),
        Value::I32(Some(n)) => json!(n),
        Value::I64(Some(n)) => json!(n),
        Value::F32(Some(n)) => json!(n),
        Value::F64(Some(n)) => json!(n),
        Value::String(Some(s)) => json!(s),
        other if other.is_null() => Json::Null,
        other => json!(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_groups_errors_by_path() {
        let mut errors = ValidationErrors::new();
        errors.add_rule(
            "name",
            "length",
            "name must be at least 2 characters",
            None,
            vec![("min", Value::from(2_i64))],
        );
        errors.add("lines.1.sku", "non_empty", "sku must not be empty");
        assert_eq!(
            DetailsReporter.report(&errors).to_string(),
            concat!(
                r#"{"error":{"code":"validation_failed","details":{"#,
                r#""lines.1.sku":[{"code":"non_empty","message":"sku must not be empty","params":{}}],"#,
                r#""name":[{"code":"length","message":"name must be at least 2 characters","params":{"min":2}}]},"#,
                r#""message":"the request body failed validation"}}"#,
            )
        );
    }
}
