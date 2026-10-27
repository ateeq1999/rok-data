//! JSON Schema from an input's declaration: the field types and the field
//! rules a schema can say (`minLength`, `maximum`, `format`, …). Database
//! rules and custom rules are left out: no schema can check them.
//!
//! | Declaration | Schema |
//! |---|---|
//! | `String`, integers, floats, `bool` | `"type"`: `string`, `integer`, `number`, `boolean` |
//! | `Option<T>` | `T`'s type or `null`; not `required` |
//! | `Vec<T>` | `array` with `items` |
//! | `#[rok(nested)]` input | its own object schema (`required` by its fields) |
//! | enum input | `oneOf` its variants |
//! | a type it can't describe | `{}` (any value) |
//! | `length(min, max)` | `minLength` / `maxLength` (`minItems` / `maxItems` on an array) |
//! | `range(min, max)` | `minimum` / `maximum` |
//! | `email` | `format: email` |
//! | `non_empty` | `minLength: 1` (`minItems: 1`) |
//! | conditional, database and custom rules | — |

use std::marker::PhantomData;

/// A JSON Schema (2020-12) node, built by `#[derive(Input)]` and turned
/// into JSON by `Input::json_schema` (feature `json`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Schema {
    ty: Option<&'static str>,
    format: Option<&'static str>,
    min_length: Option<i64>,
    max_length: Option<i64>,
    minimum: Option<f64>,
    maximum: Option<f64>,
    items: Option<Box<Schema>>,
    properties: Vec<(&'static str, Schema)>,
    required: Vec<&'static str>,
    one_of: Vec<Schema>,
    /// `null` is a valid value too (an `Option` field).
    nullable: bool,
}

impl Schema {
    /// A schema of JSON type `ty` (`"string"`, `"integer"`, …).
    pub fn of(ty: &'static str) -> Self {
        Self {
            ty: Some(ty),
            ..Self::default()
        }
    }

    /// Any value: the schema of a type it can't describe.
    pub fn any() -> Self {
        Self::default()
    }

    /// An array of `items`.
    pub fn array(items: Schema) -> Self {
        Self {
            ty: Some("array"),
            items: Some(Box::new(items)),
            ..Self::default()
        }
    }

    /// An object with `properties`, `required` naming the ones that must
    /// be present.
    #[doc(hidden)]
    pub fn object(properties: Vec<(&'static str, Schema)>, required: Vec<&'static str>) -> Self {
        Self {
            ty: Some("object"),
            properties,
            required,
            ..Self::default()
        }
    }

    /// One of `variants`.
    #[doc(hidden)]
    pub fn one_of(variants: Vec<Schema>) -> Self {
        Self {
            one_of: variants,
            ..Self::default()
        }
    }

    /// This schema, also allowing `null`.
    #[must_use]
    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }

    /// Whether `null` is allowed (the field is optional).
    pub fn is_nullable(&self) -> bool {
        self.nullable
    }

    /// `format`: `email`, `date-time`, …
    #[must_use]
    pub fn format(mut self, format: &'static str) -> Self {
        self.format = Some(format);
        self
    }

    /// A length rule: `minItems` / `maxItems` on an array, `minLength` /
    /// `maxLength` otherwise.
    #[doc(hidden)]
    #[must_use]
    pub fn length(mut self, min: Option<i64>, max: Option<i64>) -> Self {
        self.min_length = min.or(self.min_length);
        self.max_length = max.or(self.max_length);
        self
    }

    /// A range rule: `minimum` / `maximum`.
    #[doc(hidden)]
    #[must_use]
    pub fn range(mut self, min: Option<f64>, max: Option<f64>) -> Self {
        self.minimum = min.or(self.minimum);
        self.maximum = max.or(self.maximum);
        self
    }

    /// The schema as JSON.
    #[cfg(any(feature = "json", feature = "axum"))]
    pub fn to_json(&self) -> serde_json::Value {
        use serde_json::{Map, Value, json};
        let mut out = Map::new();
        match (self.ty, self.nullable) {
            (Some(ty), true) => {
                out.insert("type".into(), json!([ty, "null"]));
            }
            (Some(ty), false) => {
                out.insert("type".into(), json!(ty));
            }
            (None, _) => {}
        }
        if let Some(format) = self.format {
            out.insert("format".into(), json!(format));
        }
        let array = self.ty == Some("array");
        let (min_key, max_key) = if array {
            ("minItems", "maxItems")
        } else {
            ("minLength", "maxLength")
        };
        if let Some(min) = self.min_length {
            out.insert(min_key.into(), json!(min));
        }
        if let Some(max) = self.max_length {
            out.insert(max_key.into(), json!(max));
        }
        if let Some(min) = self.minimum {
            out.insert("minimum".into(), json!(min));
        }
        if let Some(max) = self.maximum {
            out.insert("maximum".into(), json!(max));
        }
        if let Some(items) = &self.items {
            out.insert("items".into(), items.to_json());
        }
        if self.ty == Some("object") {
            let properties: Map<String, Value> = self
                .properties
                .iter()
                .map(|(name, schema)| ((*name).to_owned(), schema.to_json()))
                .collect();
            out.insert("properties".into(), Value::Object(properties));
            out.insert("required".into(), json!(self.required));
        }
        if !self.one_of.is_empty() {
            let variants: Vec<Value> = self.one_of.iter().map(Schema::to_json).collect();
            out.insert("oneOf".into(), Value::Array(variants));
        }
        Value::Object(out)
    }
}

/// A field type's schema.
pub trait SchemaType {
    /// The schema of a value of this type.
    fn schema() -> Schema;
}

macro_rules! schema_types {
    ($($ty:ty => $json:literal),* $(,)?) => {
        $(impl SchemaType for $ty {
            fn schema() -> Schema {
                Schema::of($json)
            }
        })*
    };
}

schema_types! {
    String => "string", &str => "string", char => "string",
    i8 => "integer", i16 => "integer", i32 => "integer", i64 => "integer",
    u8 => "integer", u16 => "integer", u32 => "integer", u64 => "integer",
    f32 => "number", f64 => "number",
    bool => "boolean",
}

impl<T: SchemaType> SchemaType for Option<T> {
    fn schema() -> Schema {
        T::schema().nullable()
    }
}

impl<T: SchemaType> SchemaType for Vec<T> {
    fn schema() -> Schema {
        Schema::array(T::schema())
    }
}

#[cfg(feature = "uuid")]
impl SchemaType for sqlx::types::Uuid {
    fn schema() -> Schema {
        Schema::of("string").format("uuid")
    }
}

#[cfg(feature = "chrono")]
impl SchemaType for sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc> {
    fn schema() -> Schema {
        Schema::of("string").format("date-time")
    }
}

#[cfg(feature = "chrono")]
impl SchemaType for sqlx::types::chrono::NaiveDate {
    fn schema() -> Schema {
        Schema::of("string").format("date")
    }
}

/// Picks a field type's schema: [`SchemaType`]'s when the type has one,
/// [`Schema::any`] otherwise — `(&Probe::<T>::new()).schema()` with both
/// traits in scope.
#[doc(hidden)]
pub struct Probe<T>(PhantomData<fn() -> T>);

impl<T> Probe<T> {
    #[doc(hidden)]
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> std::fmt::Debug for Probe<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Probe")
    }
}

impl<T> Default for Probe<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// The described case of [`Probe`].
#[doc(hidden)]
pub trait ProbeTyped {
    fn schema(&self) -> Schema;
}

impl<T: SchemaType> ProbeTyped for Probe<T> {
    fn schema(&self) -> Schema {
        T::schema()
    }
}

/// The fallback case of [`Probe`].
#[doc(hidden)]
pub trait ProbeAny {
    fn schema(&self) -> Schema;
}

impl<T> ProbeAny for &Probe<T> {
    fn schema(&self) -> Schema {
        Schema::any()
    }
}
