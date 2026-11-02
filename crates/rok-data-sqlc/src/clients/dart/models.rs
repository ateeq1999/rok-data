//! The model declarations of `lib/src/models.dart`, built from the
//! document's schemas: a class per object schema (an `allOf` flattened
//! into one), an enum per string enum, a sealed class per `oneOf` of
//! objects, a `typedef` per other component. An inline object takes the
//! component class of the same shape when there is one, else a class
//! named after where it is.

use std::collections::{BTreeMap, BTreeSet};

use heck::ToUpperCamelCase;

use super::super::openapi::{Body, Document, Property, Response, Schema};
use super::Error;
use super::names::{class_name, enum_value, field_name, is_reserved_type};
use super::ty::Ty;

/// A declaration of `models.dart`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Decl {
    /// A class read from a JSON object.
    Class(Class),
    /// An enum of strings: each value's identifier and wire value.
    Enum {
        name: String,
        values: Vec<(String, String)>,
    },
    /// A sealed class: each variant class and the keys telling it apart.
    Union {
        name: String,
        variants: Vec<(String, Vec<String>)>,
    },
    /// `typedef name = ty;`.
    Alias { name: String, ty: Ty },
}

impl Decl {
    pub fn name(&self) -> &str {
        match self {
            Decl::Class(class) => &class.name,
            Decl::Enum { name, .. } | Decl::Union { name, .. } | Decl::Alias { name, .. } => name,
        }
    }
}

/// A class of `models.dart`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Class {
    pub name: String,
    pub fields: Vec<Field>,
    /// The sealed classes it is a variant of.
    pub implements: Vec<String>,
}

/// A field of a class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Field {
    /// Its Dart name.
    pub name: String,
    /// Its JSON key.
    pub key: String,
    /// Its type: nullable when the JSON may leave it out or hold `null`.
    pub ty: Ty,
    /// Whether the JSON must hold it.
    pub required: bool,
    /// A request's optional, nullable field: `Nullable<T>?`, so that
    /// leaving it out and sending `null` stay apart.
    pub tristate: bool,
    pub doc: Option<String>,
}

/// How a component is declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Class,
    Enum,
    Union,
    Alias,
}

/// The models of a document, as they are built.
pub(crate) struct Models<'a> {
    document: &'a Document,
    /// Each component's Dart name and kind.
    components: BTreeMap<String, (String, Kind)>,
    /// The components only requests use: their optional nullable fields
    /// are three-state.
    input_only: BTreeSet<String>,
    /// Each alias component's type, once resolved.
    aliases: BTreeMap<String, Ty>,
    resolving: BTreeSet<String>,
    /// The sealed classes each class is a variant of.
    variants_of: BTreeMap<String, Vec<String>>,
    pub decls: Vec<Decl>,
    taken: BTreeSet<String>,
}

impl<'a> Models<'a> {
    /// The models of every component schema of `document`; `taken` holds
    /// the names the client takes besides them.
    pub fn of(document: &'a Document, taken: BTreeSet<String>) -> Result<Models<'a>, Error> {
        let mut models = Models {
            document,
            components: BTreeMap::new(),
            input_only: input_only(document),
            aliases: BTreeMap::new(),
            resolving: BTreeSet::new(),
            variants_of: BTreeMap::new(),
            decls: Vec::new(),
            taken,
        };
        for (name, schema) in &document.schemas {
            let dart = class_name(name);
            if is_reserved_type(&dart) || !models.taken.insert(dart.clone()) {
                return Err(Error(format!(
                    "the schema `{name}` is `{dart}` in Dart, a name the client takes already; \
                     rename the Rust type (or its `#[schema(as = ...)]`)"
                )));
            }
            models.components.insert(name.clone(), (dart, kind(schema)));
        }
        for (name, schema) in &document.schemas {
            let (dart, kind) = models.components[name].clone();
            let at = format!("`#/components/schemas/{name}`");
            let input = models.input_only.contains(name);
            match kind {
                Kind::Class => {
                    let properties = models.properties(schema, &at)?;
                    models.class(&dart, &properties, input, &at)?;
                }
                Kind::Enum => models.enumeration(&dart, schema, &at)?,
                Kind::Union => {
                    let Schema::OneOf(variants) = schema else {
                        continue;
                    };
                    if variants.contains(&Schema::Null) {
                        return Err(unsupported(&at, "a `oneOf` of several objects or `null`"));
                    }
                    models.union(&dart, variants, input, &at)?;
                }
                Kind::Alias => {
                    let ty = models.alias(name)?;
                    models.decls.push(Decl::Alias { name: dart, ty });
                }
            }
        }
        Ok(models)
    }

    /// The declarations, in name order, each class naming the sealed
    /// classes it is a variant of.
    pub fn finish(mut self) -> Vec<Decl> {
        for decl in &mut self.decls {
            if let Decl::Class(class) = decl
                && let Some(unions) = self.variants_of.get(&class.name)
            {
                class.implements.clone_from(unions);
            }
        }
        self.decls.sort_by(|a, b| a.name().cmp(b.name()));
        self.decls
    }

    /// The type of `schema`, found at `at`; an inline object, union or
    /// enum is declared as `context`.
    pub fn ty(
        &mut self,
        schema: &Schema,
        context: &str,
        input: bool,
        at: &str,
    ) -> Result<Ty, Error> {
        Ok(match schema {
            Schema::Ref(name) => match self.components.get(name).cloned() {
                Some((dart, Kind::Class | Kind::Union)) => Ty::Class(dart),
                Some((dart, Kind::Enum)) => Ty::Enum(dart),
                Some((_, Kind::Alias)) => self.alias(name)?,
                None => return Err(Error(format!("{at}: no schema `{name}`"))),
            },
            Schema::Any => Ty::Any,
            Schema::Null => return Err(unsupported(at, "a value that is only ever `null`")),
            Schema::Boolean => Ty::Bool,
            Schema::Integer => Ty::Int,
            Schema::Number => Ty::Double,
            Schema::String => Ty::String,
            Schema::StringEnum(values) if values.is_empty() => Ty::String,
            Schema::StringEnum(_) => match self.component_like(schema) {
                Some(dart) => Ty::Enum(dart),
                None => {
                    let name = self.fresh(context, at)?;
                    self.enumeration(&name, schema, at)?;
                    Ty::Enum(name)
                }
            },
            Schema::Array(items) => Ty::List(Box::new(self.ty(
                items,
                &format!("{context}Item"),
                input,
                at,
            )?)),
            Schema::Object {
                properties,
                additional,
            } if properties.is_empty() => {
                let values = match additional {
                    Some(values) => self.ty(values, &format!("{context}Value"), input, at)?,
                    None => Ty::Any,
                };
                Ty::Map(Box::new(values))
            }
            Schema::Object {
                additional: Some(_),
                ..
            } => {
                return Err(unsupported(
                    at,
                    "an object with both `properties` and `additionalProperties`",
                ));
            }
            Schema::Object { properties, .. } => match self.component_like(schema) {
                Some(dart) => Ty::Class(dart),
                None => {
                    let name = self.fresh(context, at)?;
                    self.class(&name, properties, input, at)?;
                    Ty::Class(name)
                }
            },
            Schema::Nullable(inner) => self.ty(inner, context, input, at)?.nullable(),
            Schema::OneOf(variants) => {
                let real: Vec<&Schema> = variants.iter().filter(|v| **v != Schema::Null).collect();
                let nullable = real.len() < variants.len();
                let ty = match real.as_slice() {
                    [] => return Err(unsupported(at, "a `oneOf` of `null` alone")),
                    [one] => self.ty(one, context, input, at)?,
                    _ => {
                        let name = self.fresh(context, at)?;
                        let real: Vec<Schema> = real.into_iter().cloned().collect();
                        self.union(&name, &real, input, at)?;
                        Ty::Class(name)
                    }
                };
                if nullable { ty.nullable() } else { ty }
            }
            Schema::AllOf(_) => {
                let properties = self.properties(schema, at)?;
                let name = self.fresh(context, at)?;
                self.class(&name, &properties, input, at)?;
                Ty::Class(name)
            }
        })
    }

    /// `name` as a new class name, or why it is taken.
    fn fresh(&mut self, name: &str, at: &str) -> Result<String, Error> {
        let name = class_name(name);
        if is_reserved_type(&name) || !self.taken.insert(name.clone()) {
            return Err(Error(format!(
                "{at}: the inline schema would be the Dart class `{name}`, a name taken already; \
                 give it a schema of its own"
            )));
        }
        Ok(name)
    }

    /// The component of the same shape as the inline `schema`, ignoring
    /// descriptions: utoipa inlines a generic's argument (a page's items).
    /// The component it equals, descriptions included, comes first:
    /// components of one shape differ by what they document (a page's
    /// items are the `users` feature's `UserResponse`, not the account
    /// routes' `AccountResponse`).
    fn component_like(&self, schema: &Schema) -> Option<String> {
        let shape = undocumented(schema);
        let schemas = &self.document.schemas;
        schemas
            .iter()
            .find(|(_, component)| *component == schema)
            .or_else(|| {
                schemas
                    .iter()
                    .find(|(_, component)| undocumented(component) == shape)
            })
            .map(|(name, _)| self.components[name].0.clone())
    }

    /// The type an alias component stands for.
    fn alias(&mut self, name: &str) -> Result<Ty, Error> {
        if let Some(ty) = self.aliases.get(name) {
            return Ok(ty.clone());
        }
        let at = format!("`#/components/schemas/{name}`");
        if !self.resolving.insert(name.to_owned()) {
            return Err(unsupported(&at, "a schema that is an alias of itself"));
        }
        let schema = self
            .document
            .schemas
            .get(name)
            .cloned()
            .unwrap_or(Schema::Any);
        let dart = self.components[name].0.clone();
        let input = self.input_only.contains(name);
        let ty = self.ty(&schema, &format!("{dart}Value"), input, &at)?;
        self.resolving.remove(name);
        self.aliases.insert(name.to_owned(), ty.clone());
        Ok(ty)
    }

    /// The properties of an object or `allOf` schema, flattened.
    fn properties(&self, schema: &Schema, at: &str) -> Result<Vec<Property>, Error> {
        match schema {
            Schema::Object {
                properties,
                additional: None,
            } => Ok(properties.clone()),
            Schema::Ref(name) => match self.document.schemas.get(name) {
                Some(schema) => self.properties(schema, at),
                None => Err(Error(format!("{at}: no schema `{name}`"))),
            },
            Schema::AllOf(parts) => {
                let mut merged: Vec<Property> = Vec::new();
                for part in parts {
                    for property in self.properties(part, at)? {
                        merged.retain(|kept| kept.name != property.name);
                        merged.push(property);
                    }
                }
                Ok(merged)
            }
            _ => Err(unsupported(
                at,
                "an `allOf` of a schema that is not an object",
            )),
        }
    }

    /// Declares the class `name` of `properties`.
    fn class(
        &mut self,
        name: &str,
        properties: &[Property],
        input: bool,
        at: &str,
    ) -> Result<(), Error> {
        let mut fields: Vec<Field> = Vec::new();
        for property in properties {
            let context = format!("{name}{}", property.name.to_upper_camel_case());
            let here = format!("{at}, property `{}`", property.name);
            let ty = self.ty(&property.schema, &context, input, &here)?;
            let tristate = input && !property.required && ty.is_nullable() && ty != Ty::Any;
            let dart = field_name(&property.name);
            if fields.iter().any(|field| field.name == dart) {
                return Err(Error(format!(
                    "{here}: two properties are the Dart field `{dart}`; rename one"
                )));
            }
            fields.push(Field {
                name: dart,
                key: property.name.clone(),
                ty: if property.required { ty } else { ty.nullable() },
                required: property.required,
                tristate,
                doc: property.description.clone(),
            });
        }
        self.decls.push(Decl::Class(Class {
            name: name.to_owned(),

            fields,
            implements: Vec::new(),
        }));
        Ok(())
    }

    /// Declares the enum `name` of a string enum `schema`.
    fn enumeration(&mut self, name: &str, schema: &Schema, at: &str) -> Result<(), Error> {
        let Schema::StringEnum(wire) = schema else {
            return Err(unsupported(at, "an enum of values that are not strings"));
        };
        let mut values: Vec<(String, String)> = Vec::new();
        for value in wire {
            let ident = enum_value(value);
            if values.iter().any(|(taken, _)| *taken == ident) {
                return Err(Error(format!(
                    "{at}: two values are the Dart enum value `{ident}`; rename one"
                )));
            }
            values.push((ident, value.clone()));
        }
        self.decls.push(Decl::Enum {
            name: name.to_owned(),

            values,
        });
        Ok(())
    }

    /// Declares the sealed class `name` of `variants`, each an object (a
    /// component class, or an inline object declared beside it), told
    /// apart by their required keys.
    fn union(
        &mut self,
        name: &str,
        variants: &[Schema],
        input: bool,
        at: &str,
    ) -> Result<(), Error> {
        let mut out: Vec<(String, Vec<String>)> = Vec::new();
        for (index, variant) in variants.iter().enumerate() {
            let properties = match variant {
                Schema::Ref(_) | Schema::Object { .. } | Schema::AllOf(_) => {
                    self.properties(variant, at).map_err(|_| {
                        unsupported(at, "a `oneOf` with a variant that is not an object")
                    })?
                }
                _ => {
                    return Err(unsupported(
                        at,
                        "a `oneOf` with a variant that is not an object",
                    ));
                }
            };
            let class = match variant {
                Schema::Ref(component)
                    if self.components.get(component).map(|c| c.1) == Some(Kind::Class) =>
                {
                    self.components[component].0.clone()
                }
                Schema::Ref(_) => {
                    return Err(unsupported(
                        at,
                        "a `oneOf` with a variant that is not an object",
                    ));
                }
                _ => {
                    let suffix = properties
                        .iter()
                        .find(|p| p.required)
                        .or(properties.first())
                        .map_or_else(
                            || format!("Variant{}", index + 1),
                            |p| p.name.to_upper_camel_case(),
                        );
                    let mut class = class_name(&format!("{name}{suffix}"));
                    if self.taken.contains(&class) {
                        class = class_name(&format!("{name}Variant{}", index + 1));
                    }
                    let class = self.fresh(&class, at)?;
                    self.class(&class, &properties, input, at)?;
                    class
                }
            };
            let mut keys: Vec<String> = properties
                .iter()
                .filter(|p| p.required)
                .map(|p| p.name.clone())
                .collect();
            keys.sort();
            if out.iter().any(|(_, taken)| *taken == keys) {
                return Err(unsupported(
                    at,
                    "a `oneOf` whose variants require the same keys (the client could not \
                     tell them apart)",
                ));
            }
            self.variants_of
                .entry(class.clone())
                .or_default()
                .push(name.to_owned());
            out.push((class, keys));
        }
        // The variant requiring the most keys is tried first, so a variant
        // whose keys are a subset of another's does not shadow it.
        out.sort_by_key(|(_, keys)| std::cmp::Reverse(keys.len()));
        self.decls.push(Decl::Union {
            name: name.to_owned(),

            variants: out,
        });
        Ok(())
    }
}

/// How the component `schema` is declared.
fn kind(schema: &Schema) -> Kind {
    match schema {
        Schema::Object {
            properties,
            additional: None,
        } if !properties.is_empty() => Kind::Class,
        Schema::AllOf(_) => Kind::Class,
        Schema::StringEnum(values) if !values.is_empty() => Kind::Enum,
        Schema::OneOf(variants) if variants.iter().filter(|v| **v != Schema::Null).count() > 1 => {
            Kind::Union
        }
        _ => Kind::Alias,
    }
}

/// `schema` with every description dropped: its shape.
fn undocumented(schema: &Schema) -> Schema {
    match schema {
        Schema::Object {
            properties,
            additional,
        } => Schema::Object {
            properties: properties
                .iter()
                .map(|p| Property {
                    description: None,
                    schema: undocumented(&p.schema),
                    ..p.clone()
                })
                .collect(),
            additional: additional.as_ref().map(|a| Box::new(undocumented(a))),
        },
        Schema::Array(items) => Schema::Array(Box::new(undocumented(items))),
        Schema::Nullable(inner) => Schema::Nullable(Box::new(undocumented(inner))),
        Schema::OneOf(variants) => Schema::OneOf(variants.iter().map(undocumented).collect()),
        Schema::AllOf(parts) => Schema::AllOf(parts.iter().map(undocumented).collect()),
        other => other.clone(),
    }
}

/// The components only request bodies reach: no response mentions them,
/// at any depth.
fn input_only(document: &Document) -> BTreeSet<String> {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for operation in &document.operations {
        if let Some(Body::Json(schema)) = &operation.body {
            inputs.push(schema);
        }
        if let Response::Json(schema) = &operation.response {
            outputs.push(schema);
        }
    }
    let reach = |roots: Vec<&Schema>| {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut queue: Vec<String> = roots
            .into_iter()
            .flat_map(|schema| schema.refs().into_iter().map(str::to_owned))
            .collect();
        while let Some(name) = queue.pop() {
            if seen.insert(name.clone())
                && let Some(schema) = document.schemas.get(&name)
            {
                queue.extend(schema.refs().into_iter().map(str::to_owned));
            }
        }
        seen
    };
    let outputs = reach(outputs);
    reach(inputs)
        .into_iter()
        .filter(|name| !outputs.contains(name))
        .collect()
}

/// `construct`, at `at`, is outside what the Dart client reads.
fn unsupported(at: &str, construct: &str) -> Error {
    Error(format!(
        "{at}: {construct} is not supported by the Dart client"
    ))
}
