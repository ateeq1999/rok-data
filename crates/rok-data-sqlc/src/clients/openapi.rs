//! The `OpenAPI` reader the Clients are emitted from (workspace ADR 0027):
//! the OpenAPI 3.1 subset utoipa emits, and no more. [`Document::parse`]
//! turns an `openapi.json` into the model every client emitter reads —
//! the component schemas and the operations, each with its path and query
//! parameters, its request body, its success response and whether it
//! carries the bearer auth marker.
//!
//! What utoipa does not emit is refused, not guessed at: a header
//! parameter, a `multipart/form-data` body, `anyOf`, a non-bearer security
//! scheme, a `$ref` outside `#/components/schemas`... each fails the parse
//! with an [`Error::Unsupported`] naming the construct and where it is
//! (a JSON pointer). Annotations a client has no use for (descriptions,
//! examples, bounds) are read past.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Map, Value};

mod operation;
mod schema;

pub use operation::{Body, Method, Operation, Parameter, Response};
pub use schema::{Property, Schema};

/// An `openapi.json` read into the subset's model.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    /// `#/components/schemas`, by name.
    pub schemas: BTreeMap<String, Schema>,
    /// Every operation, by path and then by method, in the document's
    /// order of paths.
    pub operations: Vec<Operation>,
}

/// Why a document is not read.
#[derive(Debug)]
pub enum Error {
    /// Not JSON at all.
    Json(serde_json::Error),
    /// A construct outside the subset: what it is, and where.
    Unsupported {
        /// The JSON pointer of the construct, such as
        /// `#/paths/~1users/get/parameters/0`.
        at: String,
        /// The construct, such as "a parameter in `header`".
        construct: String,
    },
    /// A document the subset covers, but broken: a dangling `$ref`, an
    /// operation without a success response.
    Invalid {
        /// The JSON pointer of the problem.
        at: String,
        /// What is wrong.
        message: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Json(err) => write!(f, "the OpenAPI document is not JSON: {err}"),
            Error::Unsupported { at, construct } => write!(
                f,
                "{at}: {construct} is not supported (the clients read the OpenAPI 3.1 \
                 subset utoipa emits)"
            ),
            Error::Invalid { at, message } => write!(f, "{at}: {message}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Json(err) => Some(err),
            _ => None,
        }
    }
}

/// What reading a document returns.
pub type Result<T> = std::result::Result<T, Error>;

/// The keys a client reads past wherever they appear: annotations for
/// people and `x-` extensions.
fn is_annotation(key: &str) -> bool {
    matches!(
        key,
        "description" | "summary" | "title" | "example" | "examples" | "externalDocs"
    ) || key.starts_with("x-")
}

/// A JSON pointer, `at` with `key` appended (escaped per RFC 6901).
pub(crate) fn pointer(at: &str, key: &str) -> String {
    format!("{at}/{}", key.replace('~', "~0").replace('/', "~1"))
}

/// The construct `key` of the object at `at` is outside the subset.
pub(crate) fn unsupported(at: &str, key: &str, construct: impl Into<String>) -> Error {
    Error::Unsupported {
        at: pointer(at, key),
        construct: construct.into(),
    }
}

/// `value` as an object, or why not.
pub(crate) fn object<'a>(value: &'a Value, at: &str) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| Error::Invalid {
        at: at.to_owned(),
        message: "expected an object".to_owned(),
    })
}

/// Refuses every key of `map` that is neither one of `known` nor an
/// annotation, naming it.
pub(crate) fn only(map: &Map<String, Value>, at: &str, known: &[&str], what: &str) -> Result<()> {
    match map
        .keys()
        .find(|key| !known.contains(&key.as_str()) && !is_annotation(key))
    {
        Some(key) => Err(unsupported(at, key, format!("`{key}` in {what}"))),
        None => Ok(()),
    }
}

impl Document {
    /// Reads `json`, an `openapi.json`, into the model.
    ///
    /// # Errors
    ///
    /// [`Error::Json`] when it is not JSON, [`Error::Unsupported`] for a
    /// construct outside the subset, [`Error::Invalid`] for a broken
    /// document.
    pub fn parse(json: &str) -> Result<Document> {
        let root: Value = serde_json::from_str(json).map_err(Error::Json)?;
        let root = object(&root, "#")?;
        only(
            root,
            "#",
            &[
                "openapi",
                "info",
                "servers",
                "paths",
                "components",
                "security",
                "tags",
            ],
            "the document",
        )?;
        let version = root.get("openapi").and_then(Value::as_str).unwrap_or("");
        if !version.starts_with("3.1") {
            return Err(Error::Unsupported {
                at: "#/openapi".to_owned(),
                construct: format!("OpenAPI version `{version}` (3.1 is read)"),
            });
        }
        let components = match root.get("components") {
            Some(value) => object(value, "#/components")?.clone(),
            None => Map::new(),
        };
        only(
            &components,
            "#/components",
            &["schemas", "securitySchemes"],
            "the components",
        )?;
        let bearer = bearer_schemes(&components)?;
        let mut schemas = BTreeMap::new();
        if let Some(value) = components.get("schemas") {
            for (name, schema) in object(value, "#/components/schemas")? {
                let at = pointer("#/components/schemas", name);
                schemas.insert(name.clone(), Schema::parse(schema, &at)?);
            }
        }
        let default_security = match root.get("security") {
            Some(value) => Some(operation::secured(value, "#/security", &bearer)?),
            None => None,
        };
        let mut operations = Vec::new();
        if let Some(paths) = root.get("paths") {
            for (path, item) in object(paths, "#/paths")? {
                let at = pointer("#/paths", path);
                operations.extend(operation::parse_path(
                    path,
                    item,
                    &at,
                    &bearer,
                    default_security,
                )?);
            }
        }
        let document = Document {
            schemas,
            operations,
        };
        document.check_refs()?;
        Ok(document)
    }

    /// Every `$ref` names a schema the document defines.
    fn check_refs(&self) -> Result<()> {
        let check = |schema: &Schema, at: &str| -> Result<()> {
            match schema
                .refs()
                .into_iter()
                .find(|name| !self.schemas.contains_key(*name))
            {
                Some(name) => Err(Error::Invalid {
                    at: at.to_owned(),
                    message: format!(
                        "`$ref` to `{name}`, which `#/components/schemas` does not define"
                    ),
                }),
                None => Ok(()),
            }
        };
        for (name, schema) in &self.schemas {
            check(schema, &pointer("#/components/schemas", name))?;
        }
        for operation in &self.operations {
            let at = pointer("#/paths", &operation.path);
            let at = pointer(&at, operation.method.as_str());
            for parameter in operation.path_params.iter().chain(&operation.query_params) {
                check(&parameter.schema, &at)?;
            }
            if let Some(Body::Json(schema)) = &operation.body {
                check(schema, &at)?;
            }
            if let Response::Json(schema) = &operation.response {
                check(schema, &at)?;
            }
        }
        Ok(())
    }
}

/// The names of `components.securitySchemes`, each an HTTP bearer scheme:
/// the one auth marker the subset reads. Any other kind is refused.
fn bearer_schemes(components: &Map<String, Value>) -> Result<Vec<String>> {
    let Some(schemes) = components.get("securitySchemes") else {
        return Ok(Vec::new());
    };
    let at = "#/components/securitySchemes";
    let mut names = Vec::new();
    for (name, scheme) in object(schemes, at)? {
        let here = pointer(at, name);
        let scheme = object(scheme, &here)?;
        let kind = scheme.get("type").and_then(Value::as_str).unwrap_or("");
        let http = scheme.get("scheme").and_then(Value::as_str).unwrap_or("");
        if kind != "http" || !http.eq_ignore_ascii_case("bearer") {
            return Err(Error::Unsupported {
                at: here,
                construct: format!(
                    "a security scheme of type `{kind}`{} (an HTTP bearer scheme is read)",
                    if http.is_empty() {
                        String::new()
                    } else {
                        format!(" and scheme `{http}`")
                    }
                ),
            });
        }
        names.push(name.clone());
    }
    Ok(names)
}

#[cfg(test)]
mod tests;
