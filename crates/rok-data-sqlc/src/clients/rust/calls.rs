//! `src/calls.rs`: one typed `async fn` per operation, grouped by tag —
//! `client.users().update(id, &body)`. A call takes its path parameters
//! first, in the path's order, then its body, then its query.

use super::super::openapi::{Body, Parameter, Property, Response, Schema};
use super::types::Types;
use super::{Error, Named, call_name, check_groups, doc, group_method, group_type, ident};

/// The names the calls' module holds besides the groups' types.
const TAKEN: &[&str] = &["ApiError", "Client", "Method"];

/// rustfmt's widest line.
const MAX_WIDTH: usize = 100;

/// The text of `src/calls.rs` (after the header); the types the calls
/// need are defined in `types`.
pub(super) fn file(operations: &[Named<'_>], types: &mut Types<'_>) -> Result<String, Error> {
    check_groups(operations)?;
    let mut accessors = String::new();
    let mut groups = String::new();
    let mut uses_types = false;
    for members in grouped(operations) {
        let Some(first) = members.first() else {
            continue;
        };
        let tag = first.op.tag.as_deref().unwrap_or("default");
        let method = group_method(first);
        let ty = group_type(first);
        if TAKEN.contains(&ty.as_str()) {
            return Err(Error(format!(
                "the tag `{tag}` takes the name of the client's `{ty}`; rename the tag"
            )));
        }
        if !accessors.is_empty() {
            accessors.push('\n');
        }
        accessors.push_str(&format!(
            "    /// The `{tag}` operations.\n    pub fn {method}(&self) -> {ty}<'_> {{\n        \
             {ty} {{ client: self }}\n    }}\n"
        ));
        let mut calls = Vec::new();
        for named in members {
            let (text, typed) = call(named, types)?;
            uses_types |= typed;
            calls.push(text);
        }
        groups.push_str(&format!(
            "\n/// The `{tag}` operations of a [`Client`]: [`Client::{method}`].\n\
             #[derive(Debug, Clone, Copy)]\npub struct {ty}<'a> {{\n    client: &'a Client,\n}}\n\n\
             impl {ty}<'_> {{\n{}}}\n",
            calls.join("\n")
        ));
    }
    let mut out = String::from(
        "\n//! Every operation of the API, grouped by tag: `client.<tag>().<operation>(...)`.\n\n\
         use reqwest::Method;\n\n",
    );
    out.push_str(if uses_types {
        "use crate::{ApiError, Client, types};\n"
    } else {
        "use crate::{ApiError, Client};\n"
    });
    if !accessors.is_empty() {
        out.push_str(&format!("\nimpl Client {{\n{accessors}}}\n"));
    }
    out.push_str(&groups);
    Ok(out)
}

/// The operations by group, in order of first appearance.
fn grouped<'a, 'b>(operations: &'b [Named<'a>]) -> Vec<Vec<&'b Named<'a>>> {
    let mut out: Vec<Vec<&Named<'a>>> = Vec::new();
    for named in operations {
        match out
            .iter_mut()
            .find(|members| members.first().is_some_and(|m| m.group == named.group))
        {
            Some(members) => members.push(named),
            None => out.push(vec![named]),
        }
    }
    out
}

/// The name a parameter takes as an argument: an identifier none of the
/// call's other names is.
fn argument(name: &str) -> String {
    let ident = ident(&heck::ToSnakeCase::to_snake_case(name));
    if matches!(ident.as_str(), "body" | "query" | "path" | "call") {
        format!("{ident}_")
    } else {
        ident
    }
}

/// One call of a group, and whether it names one of the client's types.
fn call(named: &Named<'_>, types: &mut Types<'_>) -> Result<(String, bool), Error> {
    let op = named.op;
    let hint = format!("{}{}", group_type(named), super::type_ident(&named.name));
    let mut arguments = vec!["&self".to_owned()];
    let mut lines = Vec::new();
    for parameter in &op.path_params {
        let name = argument(&parameter.name);
        if parameter.schema == Schema::String {
            arguments.push(format!("{name}: &str"));
            lines.push(format!("let {name} = crate::segment({name});"));
        } else {
            let ty = types.ty(
                &parameter.schema,
                &format!("{hint}{}", super::type_ident(&parameter.name)),
                "types::",
            );
            arguments.push(format!("{name}: {ty}"));
            lines.push(format!("let {name} = crate::segment(&{name}.to_string());"));
        }
    }
    let mut chain = String::from("call");
    match &op.body {
        Some(Body::Json(schema)) => {
            let ty = types.input_ty(schema, &format!("{hint}Body"), "types::");
            arguments.push(format!("body: &{ty}"));
            chain.push_str(".json(body)");
        }
        Some(Body::Binary) => {
            arguments.push("body: impl Into<reqwest::Body>".to_owned());
            chain.push_str(".bytes(body)");
        }
        None => {}
    }
    if let Some(parameter) = op.query_params.iter().find(|p| !scalar(&p.schema)) {
        return Err(Error(format!(
            "`{} {}`: the query parameter `{}` is an array or an object, which the Rust \
             client does not send",
            op.method.upper(),
            op.path,
            parameter.name
        )));
    }
    if !op.query_params.is_empty() {
        let properties: Vec<Property> = op.query_params.iter().map(property).collect();
        let fallback = format!("The query of `{} {}`.", op.method.upper(), op.path);
        let name = types.define_query(&named.query_type(), &fallback, &properties);
        arguments.push(format!("query: &types::{name}"));
        chain.push_str(".query(query)");
    }
    let (answer, finish) = match &op.response {
        Response::Empty => ("()".to_owned(), ".empty_answer()"),
        Response::Binary => ("Vec<u8>".to_owned(), ".bytes_answer()"),
        Response::Json(schema) => (
            types.ty(schema, &format!("{hint}Response"), "types::"),
            ".json_answer()",
        ),
    };
    chain.push_str(finish);
    lines.push(path_line(&op.path, &op.path_params));
    lines.push(format!(
        "let call = self.client.call(Method::{}, {}path, {})?;",
        op.method.upper(),
        if op.path_params.is_empty() { "" } else { "&" },
        op.secured
    ));
    lines.push(format!("{chain}.await"));

    let summary = op.summary.as_deref();
    let fallback = format!("`{} {}`.", op.method.upper(), op.path);
    let mut text = doc(summary, &fallback, "    ");
    if op.secured {
        text.push_str("    ///\n    /// Sends the session's token.\n");
    }
    let name = call_name(named);
    let answer = format!("Result<{answer}, ApiError>");
    let one_line = format!(
        "    pub async fn {name}({}) -> {answer} {{",
        arguments.join(", ")
    );
    if one_line.len() <= MAX_WIDTH {
        text.push_str(&one_line);
        text.push('\n');
    } else {
        text.push_str(&format!("    pub async fn {name}(\n"));
        for argument in &arguments {
            text.push_str(&format!("        {argument},\n"));
        }
        text.push_str(&format!("    ) -> {answer} {{\n"));
    }
    for line in lines {
        text.push_str(&format!("        {line}\n"));
    }
    text.push_str("    }\n");
    let typed = arguments
        .iter()
        .any(|argument| argument.contains("types::"))
        || answer.contains("types::");
    Ok((text, typed))
}

/// Whether a query parameter of `schema` is one value the query encoder
/// sends: no array, no object.
fn scalar(schema: &Schema) -> bool {
    match schema {
        Schema::Array(_) | Schema::Object { .. } | Schema::AllOf(_) => false,
        Schema::Nullable(inner) => scalar(inner),
        Schema::OneOf(variants) => variants.iter().all(scalar),
        _ => true,
    }
}

/// `let path = ...;`: the path, with each parameter's (encoded) argument
/// in its place.
fn path_line(path: &str, parameters: &[Parameter]) -> String {
    let mut text = path.replace('\\', "\\\\").replace('"', "\\\"");
    if parameters.is_empty() {
        return format!("let path = \"{text}\";");
    }
    for parameter in parameters {
        text = text.replace(
            &format!("{{{}}}", parameter.name),
            &format!("{{{}}}", argument(&parameter.name)),
        );
    }
    format!("let path = format!(\"{text}\");")
}

/// A query parameter as a property of its query's struct.
fn property(parameter: &Parameter) -> Property {
    Property {
        name: parameter.name.clone(),
        schema: parameter.schema.clone(),
        required: parameter.required,
        description: parameter.description.clone(),
    }
}
