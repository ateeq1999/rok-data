//! The operations of the subset: a path's methods, each with its path and
//! query parameters, its request body, its success response and its auth
//! marker.

use serde_json::{Map, Value};

use super::{Error, Result, Schema, object, only, pointer, unsupported};

/// An HTTP method an operation answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// `GET`: a read, which the clients cache (a query).
    Get,
    /// `POST`.
    Post,
    /// `PUT`.
    Put,
    /// `PATCH`.
    Patch,
    /// `DELETE`.
    Delete,
}

impl Method {
    /// Its key in a path item: `get`, `post`...
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "get",
            Method::Post => "post",
            Method::Put => "put",
            Method::Patch => "patch",
            Method::Delete => "delete",
        }
    }

    /// Its name on the wire: `GET`, `POST`...
    pub fn upper(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }

    fn from_key(key: &str) -> Option<Method> {
        Some(match key {
            "get" => Method::Get,
            "post" => Method::Post,
            "put" => Method::Put,
            "patch" => Method::Patch,
            "delete" => Method::Delete,
            _ => return None,
        })
    }
}

/// One operation: a method of a path.
#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    /// The path, with `{name}` for each path parameter.
    pub path: String,
    /// The method.
    pub method: Method,
    /// Its `operationId`. utoipa names it after the handler, so two
    /// operations may share one.
    pub id: String,
    /// Its first tag, which groups it with its feature's operations.
    pub tag: Option<String>,
    /// Its summary, for the client's doc comment.
    pub summary: Option<String>,
    /// The path parameters, in the order the path names them.
    pub path_params: Vec<Parameter>,
    /// The query parameters, in the document's order.
    pub query_params: Vec<Parameter>,
    /// The request body, if it takes one.
    pub body: Option<Body>,
    /// What its success (its lowest `2xx`) answers.
    pub response: Response,
    /// Whether it carries the bearer auth marker: the client sends the
    /// session's token, and a `401` means the token is no longer good.
    pub secured: bool,
}

/// A path or query parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    /// Its name.
    pub name: String,
    /// Its schema.
    pub schema: Schema,
    /// Whether it must be given (every path parameter must).
    pub required: bool,
    /// Its description, for the client's doc comment.
    pub description: Option<String>,
}

/// A request body.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    /// `application/json`, of this schema.
    Json(Schema),
    /// `application/octet-stream`: raw bytes.
    Binary,
}

/// What a success answers.
#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    /// No body (a `204`, or a success documenting none).
    Empty,
    /// `application/json`, of this schema.
    Json(Schema),
    /// Any other media type (`image/jpeg`, `application/octet-stream`):
    /// raw bytes.
    Binary,
}

/// The operations of the path item `item` at `path`.
pub(super) fn parse_path(
    path: &str,
    item: &Value,
    at: &str,
    bearer: &[String],
    default_security: Option<bool>,
) -> Result<Vec<Operation>> {
    let item = object(item, at)?;
    let methods = ["get", "post", "put", "patch", "delete"];
    only(item, at, &methods, "a path item")?;
    let mut operations = Vec::new();
    for (key, value) in item {
        let Some(method) = Method::from_key(key) else {
            continue;
        };
        let here = pointer(at, key);
        operations.push(parse_operation(
            path,
            method,
            value,
            &here,
            bearer,
            default_security,
        )?);
    }
    Ok(operations)
}

fn parse_operation(
    path: &str,
    method: Method,
    value: &Value,
    at: &str,
    bearer: &[String],
    default_security: Option<bool>,
) -> Result<Operation> {
    let map = object(value, at)?;
    only(
        map,
        at,
        &[
            "tags",
            "operationId",
            "parameters",
            "requestBody",
            "responses",
            "security",
            "deprecated",
        ],
        "an operation",
    )?;
    let id = map
        .get("operationId")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Invalid {
            at: at.to_owned(),
            message: "the operation has no `operationId`".to_owned(),
        })?
        .to_owned();
    let tag = map
        .get("tags")
        .and_then(Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(Value::as_str)
        .map(str::to_owned);
    let (path_params, query_params) = parameters(map, path, at)?;
    let body = match map.get("requestBody") {
        Some(body) => Some(request_body(body, &pointer(at, "requestBody"))?),
        None => None,
    };
    let response = success(map, at)?;
    let secured = match map.get("security") {
        Some(value) => secured(value, &pointer(at, "security"), bearer)?,
        None => default_security.unwrap_or(false),
    };
    Ok(Operation {
        path: path.to_owned(),
        method,
        id,
        tag,
        summary: map
            .get("summary")
            .and_then(Value::as_str)
            .map(str::to_owned),
        path_params,
        query_params,
        body,
        response,
        secured,
    })
}

/// The operation's path parameters, in the order its path names them, and
/// its query parameters.
fn parameters(
    map: &Map<String, Value>,
    path: &str,
    at: &str,
) -> Result<(Vec<Parameter>, Vec<Parameter>)> {
    let (mut in_path, mut in_query) = (Vec::new(), Vec::new());
    if let Some(list) = map.get("parameters") {
        let at = pointer(at, "parameters");
        let list = list.as_array().ok_or_else(|| Error::Invalid {
            at: at.clone(),
            message: "expected a list of parameters".to_owned(),
        })?;
        for (index, parameter) in list.iter().enumerate() {
            let here = pointer(&at, &index.to_string());
            let parameter_map = object(parameter, &here)?;
            only(
                parameter_map,
                &here,
                &["name", "in", "required", "deprecated", "schema"],
                "a parameter",
            )?;
            let name = parameter_map
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let schema = match parameter_map.get("schema") {
                Some(schema) => Schema::parse(schema, &pointer(&here, "schema"))?,
                None => Schema::String,
            };
            let parsed = Parameter {
                name,
                schema,
                required: parameter_map
                    .get("required")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                description: parameter_map
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            };
            match parameter_map
                .get("in")
                .and_then(Value::as_str)
                .unwrap_or("")
            {
                "path" => in_path.push(parsed),
                "query" => in_query.push(parsed),
                place => {
                    return Err(Error::Unsupported {
                        at: here,
                        construct: format!("a parameter in `{place}` (path and query are read)"),
                    });
                }
            }
        }
    }
    // The order the path names them in, which is the order the client's
    // call takes them in; every `{name}` needs its parameter.
    let mut ordered = Vec::new();
    for name in path_names(path) {
        let index = in_path
            .iter()
            .position(|parameter: &Parameter| parameter.name == name)
            .ok_or_else(|| Error::Invalid {
                at: at.to_owned(),
                message: format!("the path names `{{{name}}}`, but no path parameter does"),
            })?;
        let mut parameter = in_path.remove(index);
        parameter.required = true;
        ordered.push(parameter);
    }
    if let Some(extra) = in_path.first() {
        return Err(Error::Invalid {
            at: at.to_owned(),
            message: format!("path parameter `{}` is not in the path", extra.name),
        });
    }
    Ok((ordered, in_query))
}

/// The `{name}`s of a path, in order.
fn path_names(path: &str) -> Vec<&str> {
    path.split('{')
        .skip(1)
        .filter_map(|rest| rest.split_once('}').map(|(name, _)| name))
        .collect()
}

/// The request body at `at`: JSON of a schema, or raw bytes.
fn request_body(value: &Value, at: &str) -> Result<Body> {
    let map = object(value, at)?;
    only(map, at, &["content", "required"], "a request body")?;
    let content = match map.get("content") {
        Some(content) => object(content, &pointer(at, "content"))?,
        None => {
            return Err(Error::Invalid {
                at: at.to_owned(),
                message: "the request body has no `content`".to_owned(),
            });
        }
    };
    let at = pointer(at, "content");
    if let Some(json) = content.get("application/json") {
        let here = pointer(&at, "application/json");
        return Ok(Body::Json(media_schema(json, &here)?));
    }
    match content.keys().next().map(String::as_str) {
        Some("application/octet-stream") => Ok(Body::Binary),
        Some(other) => Err(unsupported(
            &at,
            other,
            format!("a `{other}` request body (JSON and raw bytes are read)"),
        )),
        None => Err(Error::Invalid {
            at,
            message: "the request body names no media type".to_owned(),
        }),
    }
}

/// The schema of a media type object (`{"schema": ...}`); none is any JSON.
fn media_schema(value: &Value, at: &str) -> Result<Schema> {
    let map = object(value, at)?;
    only(map, at, &["schema"], "a media type")?;
    match map.get("schema") {
        Some(schema) => Schema::parse(schema, &pointer(at, "schema")),
        None => Ok(Schema::Any),
    }
}

/// What the operation's success — its lowest `2xx` — answers.
fn success(map: &Map<String, Value>, at: &str) -> Result<Response> {
    let at = pointer(at, "responses");
    let responses = match map.get("responses") {
        Some(responses) => object(responses, &at)?,
        None => {
            return Err(Error::Invalid {
                at,
                message: "the operation has no `responses`".to_owned(),
            });
        }
    };
    let Some((status, response)) = responses
        .iter()
        .filter(|(status, _)| status.len() == 3 && status.starts_with('2'))
        .min_by_key(|(status, _)| status.as_str())
    else {
        return Err(Error::Invalid {
            at,
            message: "the operation has no success (`2xx`) response".to_owned(),
        });
    };
    let at = pointer(&at, status);
    let response = object(response, &at)?;
    only(response, &at, &["content", "headers"], "a response")?;
    let Some(content) = response.get("content") else {
        return Ok(Response::Empty);
    };
    let content = object(content, &pointer(&at, "content"))?;
    if let Some(json) = content.get("application/json") {
        let here = pointer(&pointer(&at, "content"), "application/json");
        return Ok(Response::Json(media_schema(json, &here)?));
    }
    Ok(if content.is_empty() {
        Response::Empty
    } else {
        Response::Binary
    })
}

/// Whether the security requirements at `at` ask for one of the `bearer`
/// schemes. `[]` is a public operation; a requirement naming a scheme the
/// components do not define is refused.
pub(super) fn secured(value: &Value, at: &str, bearer: &[String]) -> Result<bool> {
    let list = value.as_array().ok_or_else(|| Error::Invalid {
        at: at.to_owned(),
        message: "expected a list of security requirements".to_owned(),
    })?;
    let mut secured = false;
    for (index, requirement) in list.iter().enumerate() {
        let here = pointer(at, &index.to_string());
        for name in object(requirement, &here)?.keys() {
            if !bearer.contains(name) {
                return Err(Error::Invalid {
                    at: here,
                    message: format!(
                        "the security scheme `{name}` is not one of `#/components/securitySchemes`"
                    ),
                });
            }
            secured = true;
        }
    }
    Ok(secured)
}
