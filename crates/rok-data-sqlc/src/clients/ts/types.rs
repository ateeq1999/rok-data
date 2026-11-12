//! `types.ts`: a zod schema and its type per component schema
//! (`ThingSchema`, `type Thing = z.infer<typeof ThingSchema>`), checking
//! the server's rules (`minLength`, `pattern`, `format`, …); and per
//! operation with query parameters, its query type; with header
//! parameters, its headers type (the fields its options add), as plain
//! types.

use std::collections::HashSet;

use super::super::openapi::{Body, Document, Method, Parameter, Response, Rules, Schema};
use super::{Named, TypeNames, doc, header_field, property_key};

/// The text of `types.ts` (after the header).
pub(super) fn file(document: &Document, operations: &[Named<'_>], names: &TypeNames) -> String {
    let mut out = String::from("\nimport { z } from \"zod\"\n");
    let mut declared = HashSet::new();
    let plain = plain_inputs(document);
    for (name, schema) in ordered(document) {
        let zod = Zod {
            names,
            declared: Some(&declared),
            plain: plain.contains(name.as_str()),
        };
        let type_name = names.schema(name);
        out.push_str(&format!(
            "\nexport const {} = {}\nexport type {type_name} = z.infer<typeof {}>\n",
            schema_const(&type_name),
            zod.render(schema, ""),
            schema_const(&type_name),
        ));
        declared.insert(name.as_str());
    }
    for named in operations {
        if named.op.query_params.is_empty() {
            continue;
        }
        out.push('\n');
        out.push_str(&format!(
            "/** The query of `{} {}`. */\nexport type {} = {}\n",
            named.op.method.upper(),
            named.op.path,
            named.query_type(),
            parameters(&named.op.query_params, names, property_key)
        ));
    }
    for named in operations {
        if named.op.header_params.is_empty() {
            continue;
        }
        out.push('\n');
        out.push_str(&format!(
            "/** The headers of `{} {}`. */\nexport type {} = {}\n",
            named.op.method.upper(),
            named.op.path,
            named.headers_type(),
            parameters(&named.op.header_params, names, header_field)
        ));
    }
    out
}

/// The const holding the zod schema of the type `type_name`.
pub(super) fn schema_const(type_name: &str) -> String {
    format!("{type_name}Schema")
}

/// The document's schemas, each after the schemas it refers to (a cycle
/// broken where it closes), otherwise by name.
fn ordered(document: &Document) -> Vec<(&String, &Schema)> {
    fn visit<'a>(
        name: &'a String,
        document: &'a Document,
        seen: &mut HashSet<&'a str>,
        out: &mut Vec<(&'a String, &'a Schema)>,
    ) {
        // Marked on entry, so a cycle stops where it closes.
        if !seen.insert(name.as_str()) {
            return;
        }
        let Some(schema) = document.schemas.get(name) else {
            return;
        };
        for next in schema.refs() {
            if let Some((next, _)) = document.schemas.get_key_value(next) {
                visit(next, document, seen, out);
            }
        }
        out.push((name, schema));
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for name in document.schemas.keys() {
        visit(name, document, &mut seen, &mut out);
    }
    out
}

/// The component schemas only non-`PATCH` request bodies reach: no
/// response and no `PATCH` body does. Their optional nullable fields are
/// plain optional, as leaving one out and sending `null` mean the same.
fn plain_inputs(document: &Document) -> HashSet<String> {
    let reach = |roots: Vec<&Schema>| {
        let mut seen: HashSet<String> = HashSet::new();
        let mut pending: Vec<String> = roots
            .into_iter()
            .flat_map(|schema| schema.refs().into_iter().map(str::to_owned))
            .collect();
        while let Some(name) = pending.pop() {
            if seen.insert(name.clone())
                && let Some(schema) = document.schemas.get(&name)
            {
                pending.extend(schema.refs().into_iter().map(str::to_owned));
            }
        }
        seen
    };
    let (mut sent, mut patched, mut answered) = (Vec::new(), Vec::new(), Vec::new());
    for operation in &document.operations {
        if let Some(Body::Json(schema)) = &operation.body {
            if operation.method == Method::Patch {
                patched.push(schema);
            } else {
                sent.push(schema);
            }
        }
        if let Response::Json(schema) = &operation.response {
            answered.push(schema);
        }
    }
    let (patched, answered) = (reach(patched), reach(answered));
    reach(sent)
        .into_iter()
        .filter(|name| !patched.contains(name) && !answered.contains(name))
        .collect()
}

/// `schema` without its `null`: a nullable schema's inner one, a `oneOf`'s
/// other branches.
fn without_null(schema: &Schema) -> Schema {
    match schema {
        Schema::Nullable(inner) => (**inner).clone(),
        Schema::OneOf(variants) if variants.contains(&Schema::Null) => {
            let rest: Vec<Schema> = variants
                .iter()
                .filter(|variant| **variant != Schema::Null)
                .cloned()
                .collect();
            match rest.as_slice() {
                [only] => only.clone(),
                _ => Schema::OneOf(rest),
            }
        }
        _ => schema.clone(),
    }
}

/// The other branch of a `oneOf` of `null` and one schema, as utoipa writes
/// an `Option<T>` of a component: read as `T | null`, the way a nullable
/// type list is.
fn null_or_one(schema: &Schema) -> Option<&Schema> {
    match schema {
        Schema::OneOf(variants) if variants.len() == 2 && variants.contains(&Schema::Null) => {
            variants.iter().find(|variant| **variant != Schema::Null)
        }
        _ => None,
    }
}

/// Renders zod schemas. `declared` holds the component schemas whose
/// consts exist where the text goes: a reference to another one is
/// deferred, by a getter in an object (which keeps a recursive type
/// inferable) or by `z.lazy` elsewhere. `None` means every const exists.
pub(super) struct Zod<'a> {
    /// The type names of the component schemas.
    pub names: &'a TypeNames,
    /// The component schemas already declared, or `None` for all.
    pub declared: Option<&'a HashSet<&'a str>>,
    /// Rendering a body only non-`PATCH` requests send: an optional field
    /// left out and one sent as `null` mean the same, so it is plain
    /// optional (`field?: T`).
    pub plain: bool,
}

impl Zod<'_> {
    /// Whether every schema `schema` refers to is declared.
    fn ready(&self, schema: &Schema) -> bool {
        self.declared
            .is_none_or(|declared| schema.refs().iter().all(|name| declared.contains(name)))
    }

    /// The zod schema of `schema`, multi-line objects indented for a place
    /// at `indent`.
    pub fn render(&self, schema: &Schema, indent: &str) -> String {
        if let Some(inner) = null_or_one(schema) {
            return format!("{}.nullable()", self.render(inner, indent));
        }
        match schema {
            Schema::Ref(name) => {
                let constant = schema_const(&self.names.schema(name));
                if self.ready(schema) {
                    constant
                } else {
                    format!("z.lazy(() => {constant})")
                }
            }
            Schema::Any => "z.unknown()".to_owned(),
            Schema::Null => "z.null()".to_owned(),
            Schema::Boolean => "z.boolean()".to_owned(),
            Schema::Integer(rules) => format!("z.int(){}", bounds(rules)),
            Schema::Number(rules) => format!("z.number(){}", bounds(rules)),
            Schema::String(rules) => string(rules),
            Schema::StringEnum(values) if values.is_empty() => "z.string()".to_owned(),
            Schema::StringEnum(values) => {
                let values: Vec<String> = values.iter().map(|value| literal(value)).collect();
                format!("z.enum([{}])", values.join(", "))
            }
            Schema::Array(items) => format!("z.array({})", self.render(items, indent)),
            Schema::Object {
                properties,
                additional,
            } => {
                let map = additional
                    .as_ref()
                    .map(|values| self.render(values, indent));
                if properties.is_empty() {
                    let values = map.unwrap_or_else(|| "z.unknown()".to_owned());
                    return format!("z.record(z.string(), {values})");
                }
                let inner = format!("{indent}  ");
                let mut out = String::from("z.object({\n");
                for property in properties {
                    out.push_str(&doc(property.description.as_deref(), &inner));
                    let key = property_key(&property.name);
                    let ready = self.ready(&property.schema);
                    // A schema declared later (or this one) is read in a
                    // getter, when the object is parsed.
                    let renderer = Zod {
                        names: self.names,
                        declared: if ready { self.declared } else { None },
                        plain: self.plain,
                    };
                    let schema = if self.plain && !property.required {
                        without_null(&property.schema)
                    } else {
                        property.schema.clone()
                    };
                    let mut value = renderer.render(&schema, &inner);
                    if !property.required {
                        value.push_str(".optional()");
                    }
                    if ready {
                        out.push_str(&format!("{inner}{key}: {value},\n"));
                    } else {
                        out.push_str(&format!(
                            "{inner}get {key}() {{\n{inner}  return {value}\n{inner}}},\n"
                        ));
                    }
                }
                out.push_str(&format!("{indent}}})"));
                match map {
                    Some(values) => format!("{out}.catchall({values})"),
                    None => out,
                }
            }
            Schema::Nullable(inner) => format!("{}.nullable()", self.render(inner, indent)),
            Schema::OneOf(variants) => match variants.as_slice() {
                [only] => self.render(only, indent),
                _ => {
                    let variants: Vec<String> = variants
                        .iter()
                        .map(|variant| self.render(variant, indent))
                        .collect();
                    format!("z.union([{}])", variants.join(", "))
                }
            },
            Schema::AllOf(parts) => {
                let mut parts = parts.iter().map(|part| self.render(part, indent));
                let first = parts.next().unwrap_or_else(|| "z.unknown()".to_owned());
                parts.fold(first, |all, part| format!("z.intersection({all}, {part})"))
            }
        }
    }
}

/// A JavaScript string literal of `text` (JSON's escaping is JavaScript's).
fn literal(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| format!("{text:?}"))
}

/// A number as JavaScript writes it: `2`, `0.5`.
fn number(value: f64) -> String {
    format!("{value}")
}

/// The bounds of a number: `.gte(1).lt(10)`.
fn bounds(rules: &Rules) -> String {
    let mut out = String::new();
    for (method, value) in [
        ("gte", rules.minimum),
        ("gt", rules.exclusive_minimum),
        ("lte", rules.maximum),
        ("lt", rules.exclusive_maximum),
    ] {
        if let Some(value) = value {
            out.push_str(&format!(".{method}({})", number(value)));
        }
    }
    out
}

/// A string's zod schema: its format's, then its length and pattern.
fn string(rules: &Rules) -> String {
    let mut out = match rules.format.as_deref() {
        Some("email") => "z.email()",
        Some("uuid") => "z.uuid()",
        Some("date-time") => "z.iso.datetime({ offset: true, local: true })",
        Some("date") => "z.iso.date()",
        _ => "z.string()",
    }
    .to_owned();
    if let Some(min) = rules.min_length {
        out.push_str(&format!(".min({min})"));
    }
    if let Some(max) = rules.max_length {
        out.push_str(&format!(".max({max})"));
    }
    if let Some(pattern) = &rules.pattern {
        out.push_str(&format!(".regex(new RegExp({}))", literal(pattern)));
    }
    out
}

/// An object type of `parameters`, each its `key`, optional unless
/// required.
fn parameters(parameters: &[Parameter], names: &TypeNames, key: fn(&str) -> String) -> String {
    let inner = "  ";
    let mut out = String::from("{\n");
    for parameter in parameters {
        out.push_str(&doc(parameter.description.as_deref(), inner));
        out.push_str(&format!(
            "{inner}{}{}: {}\n",
            key(&parameter.name),
            if parameter.required { "" } else { "?" },
            render(&parameter.schema, names, inner)
        ));
    }
    out.push('}');
    out
}

/// The TypeScript type of `schema`, for a call's arguments and the query
/// and header types; a multi-line object type is indented for a place at
/// `indent`.
pub(super) fn render(schema: &Schema, names: &TypeNames, indent: &str) -> String {
    if let Some(inner) = null_or_one(schema) {
        return format!("{} | null", render(inner, names, indent));
    }
    match schema {
        Schema::Ref(name) => names.schema(name),
        Schema::Any => "unknown".to_owned(),
        Schema::Null => "null".to_owned(),
        Schema::Boolean => "boolean".to_owned(),
        Schema::Integer(_) | Schema::Number(_) => "number".to_owned(),
        Schema::String(_) => "string".to_owned(),
        Schema::StringEnum(values) if values.is_empty() => "string".to_owned(),
        Schema::StringEnum(values) => values
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join(" | "),
        Schema::Array(items) => {
            let items = render(items, names, indent);
            if is_simple(&items) {
                format!("{items}[]")
            } else {
                format!("Array<{items}>")
            }
        }
        Schema::Object {
            properties,
            additional,
        } => {
            let map = additional
                .as_ref()
                .map(|values| format!("Record<string, {}>", render(values, names, indent)));
            if properties.is_empty() {
                return map.unwrap_or_else(|| "Record<string, unknown>".to_owned());
            }
            let inner = format!("{indent}  ");
            let mut out = String::from("{\n");
            for property in properties {
                out.push_str(&doc(property.description.as_deref(), &inner));
                out.push_str(&format!(
                    "{inner}{}{}: {}\n",
                    property_key(&property.name),
                    if property.required { "" } else { "?" },
                    render(&property.schema, names, &inner)
                ));
            }
            out.push_str(&format!("{indent}}}"));
            match map {
                Some(map) => format!("{out} & {map}"),
                None => out,
            }
        }
        Schema::Nullable(inner) => format!("{} | null", render(inner, names, indent)),
        Schema::OneOf(variants) => variants
            .iter()
            .map(|variant| render(variant, names, indent))
            .collect::<Vec<_>>()
            .join(" | "),
        Schema::AllOf(parts) => parts
            .iter()
            .map(|part| {
                let text = render(part, names, indent);
                if is_union(part) {
                    format!("({text})")
                } else {
                    text
                }
            })
            .collect::<Vec<_>>()
            .join(" & "),
    }
}

/// Whether a rendered type can take `[]` without parentheses.
fn is_simple(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Whether `schema` renders as a union, which an intersection must wrap.
fn is_union(schema: &Schema) -> bool {
    matches!(schema, Schema::OneOf(_) | Schema::Nullable(_))
        || matches!(schema, Schema::StringEnum(values) if values.len() > 1)
}
