//! `src/calls.ts`: `api`, one typed call per operation, grouped by tag.
//! A call takes its path parameters first, in the path's order, then its
//! body, then its query (optional when no parameter of it is required),
//! then `RequestOptions`, with a field for each header parameter
//! (`{ ifMatch: version }`), required when a header is.

use std::collections::BTreeSet;

use super::super::openapi::{Body, Response};
use super::types::{Zod, render, schema_const};
use super::{Named, TypeNames, doc, header_field, identifier};

/// One argument of a call: its name and its TypeScript parameter text.
pub(super) struct Argument {
    /// The name the call takes it by (`id`, `body`, `query`).
    pub name: String,
    /// Its type.
    pub ty: String,
    /// Whether it may be left out.
    pub optional: bool,
}

impl Argument {
    /// `name: Type`, or `name?: Type`.
    pub fn declaration(&self) -> String {
        format!(
            "{}{}: {}",
            self.name,
            if self.optional { "?" } else { "" },
            self.ty
        )
    }
}

/// The arguments of `named`'s call, options aside.
pub(super) fn arguments(named: &Named<'_>, names: &TypeNames) -> Vec<Argument> {
    let mut out = Vec::new();
    for parameter in &named.op.path_params {
        out.push(Argument {
            name: path_argument(&parameter.name),
            ty: render(&parameter.schema, names, "    "),
            optional: false,
        });
    }
    match &named.op.body {
        Some(Body::Json(schema)) => out.push(Argument {
            name: "body".to_owned(),
            ty: render(schema, names, "    "),
            optional: false,
        }),
        Some(Body::Binary) => out.push(Argument {
            name: "body".to_owned(),
            ty: "Blob".to_owned(),
            optional: false,
        }),
        None => {}
    }
    if !named.op.query_params.is_empty() {
        out.push(Argument {
            name: "query".to_owned(),
            ty: named.query_type(),
            optional: !named.op.query_params.iter().any(|p| p.required),
        });
    }
    out
}

/// Whether `field`, a header's option, is a name `RequestOptions` or
/// the call's arguments (its factory's variables) take already.
pub(super) fn taken_by_options(named: &Named<'_>, field: &str) -> bool {
    matches!(
        field,
        "signal" | "headers" | "keepSession" | "body" | "query"
    ) || named
        .op
        .path_params
        .iter()
        .any(|parameter| path_argument(&parameter.name) == field)
}

/// Whether the call's options must be given: a header it takes is
/// required.
pub(super) fn options_required(named: &Named<'_>) -> bool {
    named.op.header_params.iter().any(|p| p.required)
}

/// The type of the call's options: `RequestOptions`, with its headers
/// type when it takes header parameters.
fn options_type(named: &Named<'_>) -> String {
    if named.op.header_params.is_empty() {
        "RequestOptions".to_owned()
    } else {
        format!("RequestOptions & {}", named.headers_type())
    }
}

/// The name a path parameter takes as an argument: an identifier that is
/// not one of the call's other arguments.
fn path_argument(name: &str) -> String {
    let ident = identifier(name);
    if matches!(ident.as_str(), "body" | "query" | "options" | "variables") {
        format!("{ident}_")
    } else {
        ident
    }
}

/// The success type of `named`'s call.
pub(super) fn success_type(named: &Named<'_>, names: &TypeNames) -> String {
    match &named.op.response {
        Response::Empty => "void".to_owned(),
        Response::Binary => "Blob".to_owned(),
        Response::Json(schema) => render(schema, names, "    "),
    }
}

/// The names of the types `operations`' calls (or factories, with
/// `with_success`) mention.
pub(super) fn used_types(
    operations: &[&Named<'_>],
    names: &TypeNames,
    with_success: bool,
) -> BTreeSet<String> {
    let mut used = BTreeSet::new();
    for named in operations {
        let mut schemas = Vec::new();
        for parameter in &named.op.path_params {
            schemas.push(&parameter.schema);
        }
        if let Some(Body::Json(schema)) = &named.op.body {
            schemas.push(schema);
        }
        if with_success && let Response::Json(schema) = &named.op.response {
            schemas.push(schema);
        }
        for schema in schemas {
            used.extend(schema.refs().into_iter().map(|name| names.schema(name)));
        }
        if !named.op.query_params.is_empty() {
            used.insert(named.query_type());
        }
        if !named.op.header_params.is_empty() {
            used.insert(named.headers_type());
        }
    }
    used
}

/// `import type { ... } from "./types"`, or nothing for no types.
pub(super) fn import_types(used: &BTreeSet<String>) -> String {
    if used.is_empty() {
        return String::new();
    }
    let list = used.iter().cloned().collect::<Vec<_>>().join(",\n  ");
    format!("import type {{\n  {list},\n}} from \"./types\"\n")
}

/// The groups of `operations`, in order of first appearance.
pub(super) fn groups<'a, 'b>(
    operations: &'b [&'b Named<'a>],
) -> Vec<(&'b str, Vec<&'b Named<'a>>)> {
    let mut out: Vec<(&str, Vec<&Named<'a>>)> = Vec::new();
    for named in operations {
        match out.iter_mut().find(|(group, _)| *group == named.group) {
            Some((_, members)) => members.push(named),
            None => out.push((&named.group, vec![named])),
        }
    }
    out
}

/// The text of `src/calls.ts` (after the header).
pub(super) fn file(operations: &[Named<'_>], names: &TypeNames) -> String {
    let all: Vec<&Named<'_>> = operations.iter().collect();
    let checks: Vec<String> = all
        .iter()
        .filter_map(|named| response_schema(named, names))
        .collect();
    let mut out = String::new();
    if checks.iter().any(|check| check.contains("z.")) {
        out.push_str("\nimport { z } from \"zod\"\n");
    }
    out.push_str("\nimport { request } from \"./runtime\"\n");
    out.push_str("import type { RequestOptions } from \"./runtime\"\n");
    out.push_str(&import_types(&used_types(&all, names, true)));
    let constants: BTreeSet<String> = all
        .iter()
        .filter_map(|named| match &named.op.response {
            Response::Json(schema) => Some(schema.refs()),
            _ => None,
        })
        .flatten()
        .map(|name| schema_const(&names.schema(name)))
        .collect();
    if !constants.is_empty() {
        let list = constants.into_iter().collect::<Vec<_>>().join(",\n  ");
        out.push_str(&format!("import {{\n  {list},\n}} from \"./types\"\n"));
    }
    out.push_str("\n/** Every operation of the API, grouped by tag. */\nexport const api = {\n");
    for (group, members) in groups(&all) {
        out.push_str(&format!("  {group}: {{\n"));
        for named in members {
            out.push_str(&call(named, names));
        }
        out.push_str("  },\n");
    }
    out.push_str("}\n");
    out
}

/// One call of `api`.
fn call(named: &Named<'_>, names: &TypeNames) -> String {
    let op = named.op;
    let summary = op
        .summary
        .clone()
        .unwrap_or_else(|| format!("`{} {}`", op.method.upper(), op.path));
    let mut declarations: Vec<String> = arguments(named, names)
        .iter()
        .map(Argument::declaration)
        .collect();
    declarations.push(format!(
        "options{}: {}",
        if options_required(named) { "" } else { "?" },
        options_type(named)
    ));
    let mut fields = vec![
        format!(
            "operation: {:?}",
            format!("{} {}", op.method.upper(), op.path)
        ),
        format!("method: {:?}", op.method.upper()),
        format!("path: {}", path_expression(&op.path, &op.path_params)),
    ];
    if !op.query_params.is_empty() {
        fields.push("query".to_owned());
    }
    match &op.body {
        Some(Body::Json(_)) => fields.push("json: body".to_owned()),
        Some(Body::Binary) => fields.push("blob: body".to_owned()),
        None => {}
    }
    if !op.header_params.is_empty() {
        let options = if options_required(named) {
            "options."
        } else {
            "options?."
        };
        let headers: Vec<String> = op
            .header_params
            .iter()
            .map(|p| format!("{:?}: {options}{}", p.name, header_field(&p.name)))
            .collect();
        fields.push(format!("headers: {{ {} }}", headers.join(", ")));
    }
    fields.push(format!("secured: {}", op.secured));
    fields.push(format!(
        "response: {:?}",
        match op.response {
            Response::Empty => "none",
            Response::Binary => "blob",
            Response::Json(_) => "json",
        }
    ));
    if let Some(schema) = response_schema(named, names) {
        fields.push(format!("schema: {schema}"));
    }
    format!(
        "{}    {}: (\n      {}\n    ) =>\n      request<{}>(\n        {{\n          {},\n        }},\n        options\n      ),\n",
        doc(Some(&summary), "    "),
        named.name,
        declarations.join(",\n      "),
        success_type(named, names),
        fields.join(",\n          "),
    )
}

/// The zod schema a JSON success of `named` is checked with; none for
/// another success.
fn response_schema(named: &Named<'_>, names: &TypeNames) -> Option<String> {
    match &named.op.response {
        Response::Json(schema) => Some(
            Zod {
                names,
                declared: None,
                plain: false,
            }
            .render(schema, "          "),
        ),
        _ => None,
    }
}

/// The call's path: a string, or a template literal with each `{name}`
/// replaced by its argument, encoded.
fn path_expression(path: &str, parameters: &[super::super::openapi::Parameter]) -> String {
    if parameters.is_empty() {
        return format!("{path:?}");
    }
    let mut out = path.replace('`', "\\`");
    for parameter in parameters {
        out = out.replace(
            &format!("{{{}}}", parameter.name),
            &format!(
                "${{encodeURIComponent(String({}))}}",
                path_argument(&parameter.name)
            ),
        );
    }
    format!("`{out}`")
}
