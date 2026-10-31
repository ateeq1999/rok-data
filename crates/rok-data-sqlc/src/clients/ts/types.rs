//! `src/types.ts`: a TypeScript type per component schema, and per
//! operation with query parameters, its query type.

use super::super::openapi::{Document, Parameter, Schema};
use super::{Named, TypeNames, doc, property_key};

/// The text of `src/types.ts` (after the header).
pub(super) fn file(document: &Document, operations: &[Named<'_>], names: &TypeNames) -> String {
    let mut out = String::new();
    for (name, schema) in &document.schemas {
        out.push('\n');
        out.push_str(&format!(
            "export type {} = {}\n",
            names.schema(name),
            render(schema, names, "")
        ));
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
            parameters(&named.op.query_params, names, "")
        ));
    }
    out
}

/// An object type of `parameters`, each optional unless required.
fn parameters(parameters: &[Parameter], names: &TypeNames, indent: &str) -> String {
    let inner = format!("{indent}  ");
    let mut out = String::from("{\n");
    for parameter in parameters {
        out.push_str(&doc(parameter.description.as_deref(), &inner));
        out.push_str(&format!(
            "{inner}{}{}: {}\n",
            property_key(&parameter.name),
            if parameter.required { "" } else { "?" },
            render(&parameter.schema, names, &inner)
        ));
    }
    out.push_str(&format!("{indent}}}"));
    out
}

/// The TypeScript type of `schema`; a multi-line object type is indented
/// for a place at `indent`.
pub(super) fn render(schema: &Schema, names: &TypeNames, indent: &str) -> String {
    match schema {
        Schema::Ref(name) => names.schema(name),
        Schema::Any => "unknown".to_owned(),
        Schema::Null => "null".to_owned(),
        Schema::Boolean => "boolean".to_owned(),
        Schema::Integer | Schema::Number => "number".to_owned(),
        Schema::String => "string".to_owned(),
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
