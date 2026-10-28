//! Web integration: serde support (feature `serde`) and axum responses
//! (feature `axum`).
//!
//! With `axum`, handlers can return `rok_data::Result<T>` (or use `?` on rok-data
//! calls) and errors become JSON responses in the wire standard's shape,
//! `{"error": {"code", "message", "details"?}}` — the generated APIs' body
//! (`details` from [`DetailsReporter`](crate::validate::DetailsReporter)):
//!
//! | error | status | `error.code` |
//! |---|---|---|
//! | `NotFound` | 404 | `not_found` |
//! | `Validation` | 422 | `validation_failed` (with `details` by field path) |
//! | `Hook` | 422 | `rejected` |
//! | foreign-key violation | 422 | `invalid_reference` |
//! | `Conflict` | 409 | `conflict` |
//! | unique violation | 409 | `already_exists` |
//! | `InvalidCursor` | 400 | `invalid_cursor` |
//! | anything else | 500 | `internal_error` (details are logged, not returned) |
//!
//! ```ignore
//! async fn show(State(db): State<Db>, Path(id): Path<i64>) -> rok_data::Result<Json<User>> {
//!     Ok(Json(User::find_or_fail(&db, id).await?))
//! }
//! ```

mod serde_impls {
    use serde::ser::SerializeStruct;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use crate::validate::{FieldError, ValidationErrors};
    use crate::{Cursor, CursorPage, Page};

    /// `{"items": […], "total", "page", "per_page", "total_pages"}`
    impl<T: Serialize> Serialize for Page<T> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut st = s.serialize_struct("Page", 5)?;
            st.serialize_field("items", &self.items)?;
            st.serialize_field("total", &self.total)?;
            st.serialize_field("page", &self.page)?;
            st.serialize_field("per_page", &self.per_page)?;
            st.serialize_field("total_pages", &self.total_pages())?;
            st.end()
        }
    }

    /// `{"items": […], "next": "<cursor>" | null, "prev": "<cursor>" | null}`
    impl<T: Serialize> Serialize for CursorPage<T> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut st = s.serialize_struct("CursorPage", 3)?;
            st.serialize_field("items", &self.items)?;
            st.serialize_field("next", &self.next)?;
            st.serialize_field("prev", &self.prev)?;
            st.end()
        }
    }

    /// Serialized as its opaque string form.
    impl Serialize for Cursor {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.collect_str(self)
        }
    }

    /// Parsed from its string form, e.g. a `?after=` query parameter.
    impl<'de> Deserialize<'de> for Cursor {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let s = String::deserialize(d)?;
            s.parse().map_err(serde::de::Error::custom)
        }
    }

    impl Serialize for FieldError {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut st = s.serialize_struct("FieldError", 3)?;
            st.serialize_field("field", &self.field)?;
            st.serialize_field("code", self.code)?;
            st.serialize_field("message", &self.message)?;
            st.end()
        }
    }

    /// The wire standard's `details`: each failing path's errors, in the
    /// order the paths first failed, as `{"code", "message", "params"}`.
    impl Serialize for ValidationErrors {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeMap;
            let mut paths: Vec<&str> = Vec::new();
            for error in self.errors() {
                if !paths.contains(&error.field.as_str()) {
                    paths.push(&error.field);
                }
            }
            let mut map = s.serialize_map(Some(paths.len()))?;
            for path in paths {
                let entries: Vec<Entry<'_>> = self.field(path).map(Entry).collect();
                map.serialize_entry(path, &entries)?;
            }
            map.end()
        }
    }

    /// One error of a path in `details`.
    struct Entry<'a>(&'a FieldError);

    impl Serialize for Entry<'_> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut st = s.serialize_struct("Entry", 3)?;
            st.serialize_field("code", self.0.code)?;
            st.serialize_field("message", &self.0.message)?;
            let params: std::collections::BTreeMap<&str, Param<'_>> = self
                .0
                .params
                .iter()
                .map(|(name, value)| (*name, Param(value)))
                .collect();
            st.serialize_field("params", &params)?;
            st.end()
        }
    }

    /// A parameter as its JSON type.
    struct Param<'a>(&'a crate::Value);

    impl Serialize for Param<'_> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            use crate::Value;
            match self.0 {
                Value::Bool(Some(b)) => s.serialize_bool(*b),
                Value::I16(Some(n)) => s.serialize_i16(*n),
                Value::I32(Some(n)) => s.serialize_i32(*n),
                Value::I64(Some(n)) => s.serialize_i64(*n),
                Value::F32(Some(n)) => s.serialize_f32(*n),
                Value::F64(Some(n)) => s.serialize_f64(*n),
                Value::String(Some(text)) => s.serialize_str(text),
                other if other.is_null() => s.serialize_none(),
                other => s.serialize_str(&other.to_string()),
            }
        }
    }

    /// The wire standard's `details` read back (a client, a test).
    impl<'de> Deserialize<'de> for ValidationErrors {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum Raw {
                Bool(bool),
                Int(i64),
                Float(f64),
                Text(String),
                Other(serde::de::IgnoredAny),
            }
            #[derive(Deserialize)]
            struct RawEntry {
                code: String,
                message: String,
                #[serde(default)]
                params: std::collections::BTreeMap<String, Raw>,
            }
            let paths: std::collections::BTreeMap<String, Vec<RawEntry>> =
                std::collections::BTreeMap::deserialize(d)?;
            let mut errors = ValidationErrors::new();
            for (path, entries) in paths {
                for entry in entries {
                    let params = entry
                        .params
                        .into_iter()
                        .filter_map(|(name, raw)| {
                            let value = match raw {
                                Raw::Bool(b) => crate::Value::from(b),
                                Raw::Int(n) => crate::Value::from(n),
                                Raw::Float(n) => crate::Value::from(n),
                                Raw::Text(text) => crate::Value::from(text),
                                Raw::Other(_) => return None,
                            };
                            Some((crate::validate::intern(&name), value))
                        })
                        .collect();
                    errors.add_rule(
                        path.clone(),
                        crate::validate::intern(&entry.code),
                        entry.message,
                        None,
                        params,
                    );
                }
            }
            Ok(errors)
        }
    }
}

#[cfg(feature = "axum")]
mod axum_impls {
    use axum_core::body::Body;
    use axum_core::response::{IntoResponse, Response};
    use http::{StatusCode, header};

    use crate::Error;

    impl Error {
        /// The HTTP status and machine-readable code this error maps to.
        pub fn http_status(&self) -> (StatusCode, &'static str) {
            match self {
                Error::NotFound { .. } => (StatusCode::NOT_FOUND, "not_found"),
                Error::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "validation_failed"),
                Error::Hook(_) => (StatusCode::UNPROCESSABLE_ENTITY, "rejected"),
                Error::Conflict { .. } => (StatusCode::CONFLICT, "conflict"),
                Error::InvalidCursor(_) => (StatusCode::BAD_REQUEST, "invalid_cursor"),
                e if e.is_not_found() => (StatusCode::NOT_FOUND, "not_found"),
                e if e.is_unique_violation() => (StatusCode::CONFLICT, "already_exists"),
                e if e.is_foreign_key_violation() => {
                    (StatusCode::UNPROCESSABLE_ENTITY, "invalid_reference")
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
            }
        }
    }

    impl IntoResponse for Error {
        fn into_response(self) -> Response {
            let (status, code) = self.http_status();
            let message = match code {
                "internal_error" => {
                    tracing::error!(target: "rok_data::http", error = %self, "internal error");
                    "internal server error".to_owned()
                }
                "already_exists" => "a record with these values already exists".to_owned(),
                "invalid_reference" => "a referenced record does not exist".to_owned(),
                _ => self.to_string(),
            };
            let body = match &self {
                Error::Validation(errors) => {
                    use crate::validate::ErrorReporter;
                    crate::validate::DetailsReporter.report(errors)
                }
                _ => serde_json::json!({ "error": { "code": code, "message": message } }),
            };
            let mut response = Response::new(Body::from(body.to_string()));
            *response.status_mut() = status;
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("application/json"),
            );
            response
        }
    }
}
