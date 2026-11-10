//! `src/types.rs`: a Rust type per component schema, per operation's
//! query, and per inline schema a type needs a name for (an inline object,
//! enum or `oneOf`). An inline object of the same shape as a component
//! schema (descriptions aside) is that schema: utoipa inlines a generic's
//! argument, such as a page's items.

use std::collections::{BTreeMap, BTreeSet};

use super::super::openapi::{Body, Document, Property, Schema};
use super::{Error, RESERVED_TYPES, doc, ident, type_ident};

/// The types of the client, as the calls ask for them.
pub(crate) struct Types<'d> {
    document: &'d Document,
    /// Each component schema's Rust name.
    names: BTreeMap<String, String>,
    /// Every type name given so far.
    taken: BTreeSet<String>,
    /// The definitions, in the order they were given.
    defs: Vec<String>,
    /// The component schemas a request body reaches: their properties
    /// that may be left out and may be `null` tell the two apart.
    inputs: BTreeSet<String>,
    /// Whether the types being defined are a request's (see `inputs`).
    input: bool,
}

impl<'d> Types<'d> {
    /// The types of `document`'s component schemas.
    pub fn new(document: &'d Document) -> Result<Types<'d>, Error> {
        let mut types = Types {
            document,
            names: BTreeMap::new(),
            taken: BTreeSet::new(),
            defs: Vec::new(),
            inputs: inputs(document),
            input: false,
        };
        for name in document.schemas.keys() {
            let rust = type_ident(name);
            if RESERVED_TYPES.contains(&rust.as_str()) {
                return Err(Error(format!(
                    "the schema `{name}` takes the name of the client's `{rust}`; rename the \
                     Rust type (or its `#[schema(as = ...)]`)"
                )));
            }
            if !types.taken.insert(rust.clone()) {
                return Err(Error(format!(
                    "two schemas are named `{rust}` in Rust; rename one"
                )));
            }
            types.names.insert(name.clone(), rust);
        }
        for (name, schema) in &document.schemas {
            types.input = types.inputs.contains(name);
            types.component(name, schema);
        }
        types.input = false;
        Ok(types)
    }

    /// The Rust type of a request body's `schema`, as [`Types::ty`].
    pub fn input_ty(&mut self, schema: &Schema, hint: &str, prefix: &str) -> String {
        self.input = true;
        let ty = self.ty(schema, hint, prefix);
        self.input = false;
        ty
    }

    /// `src/types.rs`, after the header.
    pub fn finish(self) -> String {
        let mut out = String::from(
            "\n//! The API's types, as its `openapi.json` documents them: what the calls \
             take and answer.\n",
        );
        if !self.defs.is_empty() {
            out.push_str("\nuse serde::{Deserialize, Serialize};\n");
        }
        for def in &self.defs {
            out.push('\n');
            out.push_str(def);
        }
        out
    }

    /// A type name from `hint` that no type has taken yet.
    fn fresh(&mut self, hint: &str) -> String {
        let base = type_ident(hint);
        let mut name = base.clone();
        let mut n = 2;
        while RESERVED_TYPES.contains(&name.as_str()) || self.taken.contains(&name) {
            name = format!("{base}{n}");
            n += 1;
        }
        self.taken.insert(name.clone());
        name
    }

    /// The Rust name of the component schema `name`.
    fn named(&self, name: &str) -> String {
        self.names
            .get(name)
            .cloned()
            .unwrap_or_else(|| type_ident(name))
    }

    /// Defines the component schema `name`.
    fn component(&mut self, name: &str, schema: &Schema) {
        let rust = self.named(name);
        let fallback = format!("The API's `{name}`.");
        match schema {
            Schema::Object {
                properties,
                additional,
            } if !properties.is_empty() => {
                self.define_struct(&rust, &fallback, properties, additional.as_deref(), &[]);
            }
            Schema::StringEnum(values) if !values.is_empty() => {
                self.define_enum(&rust, &fallback, values);
            }
            Schema::OneOf(variants) => {
                let (nullable, rest) = split_null(variants);
                if !nullable && rest.len() > 1 {
                    self.define_one_of(&rust, &fallback, &rest);
                } else {
                    self.alias(&rust, &fallback, schema);
                }
            }
            Schema::AllOf(parts) => self.define_all_of(&rust, &fallback, parts),
            _ => self.alias(&rust, &fallback, schema),
        }
    }

    /// `pub type <name> = <type of schema>;`.
    fn alias(&mut self, name: &str, fallback: &str, schema: &Schema) {
        let ty = self.ty(schema, &format!("{name}Value"), "");
        self.defs.push(format!(
            "{}pub type {name} = {ty};\n",
            doc(None, fallback, "")
        ));
    }

    /// The Rust type of `schema`, prefixed with `prefix` where it names one
    /// of the client's types (`types::` from the calls); an inline type
    /// takes a name from `hint`.
    pub fn ty(&mut self, schema: &Schema, hint: &str, prefix: &str) -> String {
        match schema {
            Schema::Ref(name) => format!("{prefix}{}", self.named(name)),
            Schema::Any => "serde_json::Value".to_owned(),
            Schema::Null => "()".to_owned(),
            Schema::Boolean => "bool".to_owned(),
            Schema::Integer(_) => "i64".to_owned(),
            Schema::Number(_) => "f64".to_owned(),
            Schema::String(_) => "String".to_owned(),
            Schema::StringEnum(values) if values.is_empty() => "String".to_owned(),
            Schema::StringEnum(values) => {
                let name = self.fresh(hint);
                self.define_enum(&name, &format!("The values of `{name}`."), values);
                format!("{prefix}{name}")
            }
            Schema::Array(items) => {
                format!("Vec<{}>", self.ty(items, &format!("{hint}Item"), prefix))
            }
            Schema::Object {
                properties,
                additional,
            } if properties.is_empty() => match additional {
                Some(values) => format!(
                    "std::collections::BTreeMap<String, {}>",
                    self.ty(values, &format!("{hint}Value"), prefix)
                ),
                None => "serde_json::Map<String, serde_json::Value>".to_owned(),
            },
            Schema::Object {
                properties,
                additional,
            } => {
                if let Some(name) = self.same_as_component(schema) {
                    return format!("{prefix}{name}");
                }
                let name = self.fresh(hint);
                let fallback = format!("The `{name}` object.");
                self.define_struct(&name, &fallback, properties, additional.as_deref(), &[]);
                format!("{prefix}{name}")
            }
            Schema::Nullable(inner) => format!("Option<{}>", self.ty(inner, hint, prefix)),
            Schema::OneOf(variants) => {
                let (nullable, rest) = split_null(variants);
                let inner = match rest.as_slice() {
                    [] => "()".to_owned(),
                    [one] => self.ty(one, hint, prefix),
                    _ => {
                        let name = self.fresh(hint);
                        self.define_one_of(&name, &format!("One of `{name}`'s shapes."), &rest);
                        format!("{prefix}{name}")
                    }
                };
                if nullable {
                    format!("Option<{inner}>")
                } else {
                    inner
                }
            }
            Schema::AllOf(parts) => {
                let name = self.fresh(hint);
                self.define_all_of(&name, &format!("The `{name}` object."), parts);
                format!("{prefix}{name}")
            }
        }
    }

    /// The component schema an inline object has the shape of, if any: the
    /// one it equals, descriptions included, before one of the same shape
    /// (components of one shape differ by what they document: a page's
    /// items are the `users` feature's `UserResponse`, not the account
    /// routes' `AccountResponse`).
    fn same_as_component(&self, schema: &Schema) -> Option<String> {
        let schemas = &self.document.schemas;
        schemas
            .iter()
            .find(|(_, component)| *component == schema)
            .or_else(|| {
                schemas
                    .iter()
                    .find(|(_, component)| same_shape(component, schema))
            })
            .map(|(name, _)| self.named(name))
    }

    /// A string enum `name` of `values`, with `as_str` and `Display`.
    fn define_enum(&mut self, name: &str, fallback: &str, values: &[String]) {
        let mut variants: Vec<(String, &str)> = Vec::new();
        for value in values {
            let mut variant = type_ident(value);
            if value.is_empty() {
                variant = "Empty".to_owned();
            }
            let base = variant.clone();
            let mut n = 2;
            while variants.iter().any(|(taken, _)| *taken == variant) {
                variant = format!("{base}{n}");
                n += 1;
            }
            variants.push((variant, value));
        }
        let mut out = doc(None, fallback, "");
        out.push_str(
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]\n",
        );
        out.push_str(&format!("pub enum {name} {{\n"));
        for (variant, value) in &variants {
            out.push_str(&format!(
                "    /// `{value}`.\n    #[serde(rename = {value:?})]\n    {variant},\n"
            ));
        }
        out.push_str("}\n\n");
        out.push_str(&format!(
            "impl {name} {{\n    /// Its value on the wire.\n    \
             pub fn as_str(self) -> &'static str {{\n        match self {{\n"
        ));
        for (variant, value) in &variants {
            out.push_str(&format!("            {name}::{variant} => {value:?},\n"));
        }
        out.push_str("        }\n    }\n}\n\n");
        out.push_str(&format!(
            "impl std::fmt::Display for {name} {{\n    \
             fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n        \
             f.write_str(self.as_str())\n    }}\n}}\n"
        ));
        self.defs.push(out);
    }

    /// An untagged enum `name`, one variant per schema of `variants`.
    fn define_one_of(&mut self, name: &str, fallback: &str, variants: &[Schema]) {
        let mut out = doc(None, fallback, "");
        out.push_str("#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\n");
        out.push_str("#[serde(untagged)]\n#[allow(clippy::large_enum_variant)]\n");
        out.push_str(&format!("pub enum {name} {{\n"));
        let mut taken: Vec<String> = Vec::new();
        for (index, variant) in variants.iter().enumerate() {
            let base = variant_name(variant, index, &self.names);
            let mut label = base.clone();
            let mut n = 2;
            while taken.contains(&label) {
                label = format!("{base}{n}");
                n += 1;
            }
            let ty = self.ty(variant, &format!("{name}{label}"), "");
            out.push_str(&format!(
                "    /// The `{label}` shape.\n    {label}({ty}),\n"
            ));
            taken.push(label);
        }
        out.push_str("}\n");
        self.defs.push(out);
    }

    /// The struct `name` holding every part of `parts`: their properties
    /// merged when each is an object (or a reference to one), each part a
    /// flattened field otherwise.
    fn define_all_of(&mut self, name: &str, fallback: &str, parts: &[Schema]) {
        let mut properties: Vec<Property> = Vec::new();
        let mut additional = None;
        if self.merge(parts, &mut properties, &mut additional, 0) {
            let additional = additional.cloned();
            self.define_struct(name, fallback, &properties, additional.as_ref(), &[]);
            return;
        }
        let mut flattened = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            let field = match part {
                Schema::Ref(target) => ident(&heck::ToSnakeCase::to_snake_case(target.as_str())),
                _ => format!("part{}", index + 1),
            };
            let ty = self.ty(part, &format!("{name}Part{}", index + 1), "");
            flattened.push((field, ty));
        }
        self.define_struct(name, fallback, &[], None, &flattened);
    }

    /// Collects the properties of `parts` into `properties` (a later one
    /// replacing an earlier of its name); `false` when a part is not an
    /// object.
    fn merge<'a>(
        &'a self,
        parts: &'a [Schema],
        properties: &mut Vec<Property>,
        additional: &mut Option<&'a Schema>,
        depth: usize,
    ) -> bool {
        if depth > 8 {
            return false;
        }
        for part in parts {
            let part = match part {
                Schema::Ref(name) => match self.document.schemas.get(name) {
                    Some(schema) => schema,
                    None => return false,
                },
                other => other,
            };
            match part {
                Schema::Object {
                    properties: more,
                    additional: values,
                } => {
                    for property in more {
                        properties.retain(|kept| kept.name != property.name);
                        properties.push(property.clone());
                    }
                    if let Some(values) = values {
                        *additional = Some(values);
                    }
                }
                Schema::AllOf(inner) => {
                    if !self.merge(inner, properties, additional, depth + 1) {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        true
    }

    /// The struct `name`: a field per property, a flattened map of the
    /// other values when `additional` gives their schema, and the
    /// `flattened` fields (name, type).
    fn define_struct(
        &mut self,
        name: &str,
        fallback: &str,
        properties: &[Property],
        additional: Option<&Schema>,
        flattened: &[(String, String)],
    ) {
        let mut fields = String::new();
        let mut taken: Vec<String> = Vec::new();
        let mut all_optional = flattened.is_empty();
        for property in properties {
            let mut field = ident(&heck::ToSnakeCase::to_snake_case(property.name.as_str()));
            let base = field.clone();
            let mut n = 2;
            while taken.contains(&field) {
                field = format!("{base}{n}");
                n += 1;
            }
            taken.push(field.clone());
            let hint = format!("{name}{}", type_ident(&property.name));
            let (nullable, inner) = nullable(&property.schema);
            let base = self.ty(&inner, &hint, "");
            fields.push_str(&doc(
                property.description.as_deref(),
                &format!("The `{}` property.", property.name),
                "    ",
            ));
            if field != property.name {
                fields.push_str(&format!("    #[serde(rename = {:?})]\n", property.name));
            }
            // Only a request tells "left out" (keep the value) from `null`
            // (clear it); an answer's absent property is its `null`.
            let ty = match (
                property.required,
                nullable && (self.input || property.required),
            ) {
                (true, false) => {
                    all_optional = false;
                    base
                }
                (true, true) => format!("Option<{base}>"),
                (false, false) => {
                    fields.push_str(
                        "    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n",
                    );
                    format!("Option<{base}>")
                }
                (false, true) => {
                    fields.push_str(
                        "    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n    \
                         #[serde(deserialize_with = \"crate::nullable\")]\n",
                    );
                    format!("Option<Option<{base}>>")
                }
            };
            fields.push_str(&format!("    pub {field}: {ty},\n"));
        }
        if let Some(values) = additional {
            let mut field = "additional".to_owned();
            while taken.contains(&field) {
                field.push('_');
            }
            let ty = self.ty(values, &format!("{name}Value"), "");
            fields.push_str(&format!(
                "    /// The other properties, by name.\n    #[serde(flatten)]\n    \
                 pub {field}: std::collections::BTreeMap<String, {ty}>,\n"
            ));
        }
        for (field, ty) in flattened {
            fields.push_str(&format!(
                "    /// The `{field}` part.\n    #[serde(flatten)]\n    pub {field}: {ty},\n"
            ));
        }
        let derives = if all_optional {
            "Debug, Clone, Default, PartialEq, Serialize, Deserialize"
        } else {
            "Debug, Clone, PartialEq, Serialize, Deserialize"
        };
        self.defs.push(format!(
            "{}#[derive({derives})]\npub struct {name} {{\n{fields}}}\n",
            doc(None, fallback, "")
        ));
    }

    /// The struct of an operation's query parameters, named from `hint`
    /// (numbered when a schema has the name already): its name.
    pub fn define_query(&mut self, hint: &str, fallback: &str, properties: &[Property]) -> String {
        let name = self.fresh(hint);
        self.define_struct(&name, fallback, properties, None, &[]);
        name
    }
}

/// The component schemas a request body reaches, through any `$ref`.
fn inputs(document: &Document) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut pending: Vec<&str> = document
        .operations
        .iter()
        .filter_map(|op| match &op.body {
            Some(Body::Json(schema)) => Some(schema.refs()),
            _ => None,
        })
        .flatten()
        .collect();
    while let Some(name) = pending.pop() {
        if found.insert(name.to_owned())
            && let Some(schema) = document.schemas.get(name)
        {
            pending.extend(schema.refs());
        }
    }
    found
}

/// Whether `schema` may be `null`, and the schema of its other values.
fn nullable(schema: &Schema) -> (bool, Schema) {
    match schema {
        Schema::Nullable(inner) => (true, (**inner).clone()),
        Schema::OneOf(variants) => {
            let (nullable, rest) = split_null(variants);
            match rest.as_slice() {
                [one] if nullable => (true, one.clone()),
                _ if nullable => (true, Schema::OneOf(rest)),
                _ => (false, schema.clone()),
            }
        }
        _ => (false, schema.clone()),
    }
}

/// Whether `variants` hold `null`, and the others.
fn split_null(variants: &[Schema]) -> (bool, Vec<Schema>) {
    let rest: Vec<Schema> = variants
        .iter()
        .filter(|variant| **variant != Schema::Null)
        .cloned()
        .collect();
    (rest.len() != variants.len(), rest)
}

/// The variant name of a `oneOf`'s `variant` at `index`.
fn variant_name(variant: &Schema, index: usize, names: &BTreeMap<String, String>) -> String {
    match variant {
        Schema::Ref(name) => names.get(name).cloned().unwrap_or_else(|| type_ident(name)),
        Schema::Boolean => "Boolean".to_owned(),
        Schema::Integer(_) => "Integer".to_owned(),
        Schema::Number(_) => "Number".to_owned(),
        Schema::String(_) | Schema::StringEnum(_) => "Text".to_owned(),
        Schema::Array(_) => "List".to_owned(),
        Schema::Any => "Value".to_owned(),
        Schema::Object { properties, .. } => properties
            .iter()
            .find(|property| property.required)
            .or(properties.first())
            .map_or_else(
                || format!("Variant{}", index + 1),
                |property| type_ident(&property.name),
            ),
        _ => format!("Variant{}", index + 1),
    }
}

/// Whether `a` and `b` are the same schema, descriptions aside.
fn same_shape(a: &Schema, b: &Schema) -> bool {
    match (a, b) {
        (
            Schema::Object {
                properties: pa,
                additional: aa,
            },
            Schema::Object {
                properties: pb,
                additional: ab,
            },
        ) => {
            pa.len() == pb.len()
                && pa.iter().zip(pb).all(|(x, y)| {
                    x.name == y.name && x.required == y.required && same_shape(&x.schema, &y.schema)
                })
                && match (aa, ab) {
                    (Some(x), Some(y)) => same_shape(x, y),
                    (None, None) => true,
                    _ => false,
                }
        }
        (Schema::Array(x), Schema::Array(y)) | (Schema::Nullable(x), Schema::Nullable(y)) => {
            same_shape(x, y)
        }
        (Schema::OneOf(x), Schema::OneOf(y)) | (Schema::AllOf(x), Schema::AllOf(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same_shape(x, y))
        }
        _ => a == b,
    }
}
