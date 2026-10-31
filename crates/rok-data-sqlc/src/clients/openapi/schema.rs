//! The JSON Schemas of the subset: what utoipa writes for a Rust type
//! deriving `ToSchema`, read into [`Schema`].

use serde_json::{Map, Value};

use super::{Error, Result, object, only, pointer, unsupported};

/// A schema, as far as a client's types need it.
#[derive(Debug, Clone, PartialEq)]
pub enum Schema {
    /// `{"$ref": "#/components/schemas/<name>"}`.
    Ref(String),
    /// Any JSON value: a schema with no `type` (`{}`, or annotations only).
    Any,
    /// `{"type": "null"}`.
    Null,
    /// `{"type": "boolean"}`.
    Boolean,
    /// `{"type": "integer"}`, whatever its `format`.
    Integer,
    /// `{"type": "number"}`.
    Number,
    /// `{"type": "string"}`, whatever its `format` (`uuid`, `date-time`).
    String,
    /// A string `enum`: its values.
    StringEnum(Vec<String>),
    /// `{"type": "array", "items": ...}`.
    Array(Box<Schema>),
    /// `{"type": "object"}`: its properties and, for a map, the schema of
    /// its other values (`additionalProperties`).
    Object {
        /// The declared properties, by name.
        properties: Vec<Property>,
        /// `additionalProperties` given as a schema: a map's values.
        additional: Option<Box<Schema>>,
    },
    /// The schema, or `null`: `"type": [<type>, "null"]`.
    Nullable(Box<Schema>),
    /// `oneOf`: one of the schemas.
    OneOf(Vec<Schema>),
    /// `allOf`: all of the schemas at once.
    AllOf(Vec<Schema>),
}

/// A property of an object schema.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// Its name on the wire.
    pub name: String,
    /// Its schema.
    pub schema: Schema,
    /// Whether the object's `required` lists it.
    pub required: bool,
    /// Its description, for the client's doc comment.
    pub description: Option<String>,
}

/// The keywords that constrain a value without changing its type: a
/// client's types are the same with or without them, and the server
/// checks them.
const CONSTRAINTS: &[&str] = &[
    "format",
    "default",
    "deprecated",
    "readOnly",
    "writeOnly",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "minLength",
    "maxLength",
    "pattern",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minProperties",
    "maxProperties",
    "contentEncoding",
    "contentMediaType",
];

/// The keywords that shape a schema, which [`Schema::parse`] reads.
const SHAPES: &[&str] = &[
    "$ref",
    "type",
    "enum",
    "items",
    "properties",
    "required",
    "additionalProperties",
    "propertyNames",
    "oneOf",
    "allOf",
];

impl Schema {
    /// Reads the schema `value`, found at `at`.
    pub(crate) fn parse(value: &Value, at: &str) -> Result<Schema> {
        let map = object(value, at)?;
        let known: Vec<&str> = SHAPES.iter().chain(CONSTRAINTS).copied().collect();
        only(map, at, &known, "a schema")?;
        if let Some(reference) = map.get("$ref") {
            return reference_name(reference, at).map(Schema::Ref);
        }
        if let Some(variants) = map.get("oneOf") {
            return variants_of(variants, &pointer(at, "oneOf")).map(Schema::OneOf);
        }
        if let Some(parts) = map.get("allOf") {
            return variants_of(parts, &pointer(at, "allOf")).map(Schema::AllOf);
        }
        match map.get("type") {
            None if map.contains_key("properties") => typed(map, "object", at),
            None => Ok(Schema::Any),
            Some(Value::String(kind)) => typed(map, kind, at),
            Some(Value::Array(kinds)) => {
                let kinds: Vec<&str> = kinds.iter().filter_map(Value::as_str).collect();
                let real: Vec<&str> = kinds.iter().copied().filter(|k| *k != "null").collect();
                match (real.as_slice(), kinds.len() - real.len()) {
                    ([kind], 1) => Ok(Schema::Nullable(Box::new(typed(map, kind, at)?))),
                    ([kind], 0) => typed(map, kind, at),
                    _ => Err(unsupported(
                        at,
                        "type",
                        format!("the type list `{}`", kinds.join(", ")),
                    )),
                }
            }
            Some(other) => Err(Error::Invalid {
                at: pointer(at, "type"),
                message: format!("expected a type name, found {other}"),
            }),
        }
    }

    /// The names of the schemas this one refers to, at any depth.
    pub fn refs(&self) -> Vec<&str> {
        let mut names = Vec::new();
        self.collect_refs(&mut names);
        names
    }

    fn collect_refs<'a>(&'a self, names: &mut Vec<&'a str>) {
        match self {
            Schema::Ref(name) => names.push(name),
            Schema::Array(items) | Schema::Nullable(items) => items.collect_refs(names),
            Schema::Object {
                properties,
                additional,
            } => {
                for property in properties {
                    property.schema.collect_refs(names);
                }
                if let Some(additional) = additional {
                    additional.collect_refs(names);
                }
            }
            Schema::OneOf(variants) | Schema::AllOf(variants) => {
                for variant in variants {
                    variant.collect_refs(names);
                }
            }
            Schema::Any
            | Schema::Null
            | Schema::Boolean
            | Schema::Integer
            | Schema::Number
            | Schema::String
            | Schema::StringEnum(_) => {}
        }
    }
}

/// The schema `name` a `$ref` points at: only `#/components/schemas/<name>`
/// is read.
fn reference_name(reference: &Value, at: &str) -> Result<String> {
    let text = reference.as_str().unwrap_or("");
    match text.strip_prefix("#/components/schemas/") {
        Some(name) if !name.is_empty() && !name.contains('/') => Ok(name.to_owned()),
        _ => Err(unsupported(
            at,
            "$ref",
            format!("a `$ref` to `{text}` (only `#/components/schemas/<name>` is read)"),
        )),
    }
}

/// The schemas of a `oneOf` or `allOf` list at `at`.
fn variants_of(value: &Value, at: &str) -> Result<Vec<Schema>> {
    let list = value.as_array().ok_or_else(|| Error::Invalid {
        at: at.to_owned(),
        message: "expected a list of schemas".to_owned(),
    })?;
    list.iter()
        .enumerate()
        .map(|(index, schema)| Schema::parse(schema, &pointer(at, &index.to_string())))
        .collect()
}

/// The schema of the single type `kind`, the rest of its keywords in `map`.
fn typed(map: &Map<String, Value>, kind: &str, at: &str) -> Result<Schema> {
    if let Some(values) = map.get("enum") {
        return string_enum(values, kind, &pointer(at, "enum"));
    }
    match kind {
        "null" => Ok(Schema::Null),
        "boolean" => Ok(Schema::Boolean),
        "integer" => Ok(Schema::Integer),
        "number" => Ok(Schema::Number),
        "string" => Ok(Schema::String),
        "array" => match map.get("items") {
            Some(items) => Ok(Schema::Array(Box::new(Schema::parse(
                items,
                &pointer(at, "items"),
            )?))),
            None => Ok(Schema::Array(Box::new(Schema::Any))),
        },
        "object" => object_schema(map, at),
        other => Err(unsupported(at, "type", format!("the type `{other}`"))),
    }
}

/// A string enum's values; `null` among them is left to the type list
/// (`"type": ["string", "null"]`), which makes the schema nullable.
fn string_enum(values: &Value, kind: &str, at: &str) -> Result<Schema> {
    if kind != "string" {
        return Err(Error::Unsupported {
            at: at.to_owned(),
            construct: format!("an `enum` of type `{kind}` (string enums are read)"),
        });
    }
    let list = values.as_array().ok_or_else(|| Error::Invalid {
        at: at.to_owned(),
        message: "expected a list of values".to_owned(),
    })?;
    let mut strings = Vec::new();
    for value in list {
        match value {
            Value::String(text) => strings.push(text.clone()),
            Value::Null => {}
            other => {
                return Err(Error::Unsupported {
                    at: at.to_owned(),
                    construct: format!("the enum value {other} (string values are read)"),
                });
            }
        }
    }
    Ok(Schema::StringEnum(strings))
}

/// An object schema: its properties, which of them are required, and a
/// map's value schema.
fn object_schema(map: &Map<String, Value>, at: &str) -> Result<Schema> {
    let required: Vec<&str> = map
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut properties = Vec::new();
    if let Some(value) = map.get("properties") {
        let at = pointer(at, "properties");
        for (name, schema) in object(value, &at)? {
            let here = pointer(&at, name);
            properties.push(Property {
                name: name.clone(),
                schema: Schema::parse(schema, &here)?,
                required: required.contains(&name.as_str()),
                description: schema
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            });
        }
    }
    if let Some(name) = required
        .iter()
        .find(|name| !properties.iter().any(|property| property.name == **name))
    {
        return Err(Error::Invalid {
            at: pointer(at, "required"),
            message: format!("`{name}` is required but not a property"),
        });
    }
    let additional = match map.get("additionalProperties") {
        None | Some(Value::Bool(_)) => None,
        Some(schema) => Some(Box::new(Schema::parse(
            schema,
            &pointer(at, "additionalProperties"),
        )?)),
    };
    if let Some(names) = map.get("propertyNames") {
        // A map's keys are strings on the wire; anything narrower is a
        // constraint the server checks.
        let names = Schema::parse(names, &pointer(at, "propertyNames"))?;
        if !matches!(names, Schema::String | Schema::StringEnum(_)) {
            return Err(unsupported(
                at,
                "propertyNames",
                "`propertyNames` other than strings",
            ));
        }
    }
    Ok(Schema::Object {
        properties,
        additional,
    })
}
