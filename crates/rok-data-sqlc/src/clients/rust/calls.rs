//! `src/calls.rs`: one typed `async fn` per operation, grouped by tag —
//! `client.users().update(id, &body)`. A call takes its path parameters
//! first, in the path's order, then its body, then its query, then its
//! header parameters (`if_match`, an `Option` when the header may be left
//! out).

use super::super::openapi::{Body, Paging, Parameter, Property, Response, Schema};
use super::types::Types;
use super::{Error, Named, call_name, check_groups, doc, group_method, group_type, ident};

/// The names the calls' module holds besides the groups' types.
const TAKEN: &[&str] = &["ApiError", "Client", "Method"];

/// rustfmt's widest line.
const MAX_WIDTH: usize = 100;

/// rustfmt's widest chain kept on one line.
const CHAIN_WIDTH: usize = 60;

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
        if matches!(parameter.schema, Schema::String(_)) {
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
    let mut chain = vec!["call".to_owned()];
    match &op.body {
        Some(Body::Json(schema)) => {
            let ty = types.input_ty(schema, &format!("{hint}Body"), "types::");
            arguments.push(format!("body: &{ty}"));
            chain.push(".json(body)".to_owned());
        }
        Some(Body::Binary) => {
            arguments.push("body: impl Into<reqwest::Body>".to_owned());
            chain.push(".bytes(body)".to_owned());
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
    let mut query_type = None;
    if !op.query_params.is_empty() {
        let properties: Vec<Property> = op.query_params.iter().map(property).collect();
        let fallback = format!("The query of `{} {}`.", op.method.upper(), op.path);
        let name = types.define_query(&named.query_type(), &fallback, &properties);
        query_type = Some(name.clone());
        arguments.push(format!("query: &types::{name}"));
        chain.push(".query(query)".to_owned());
    }
    let mut header_docs = Vec::new();
    for parameter in &op.header_params {
        if !scalar(&parameter.schema) {
            return Err(Error(format!(
                "`{} {}`: the header parameter `{}` is an array or an object, which the \
                 Rust client does not send",
                op.method.upper(),
                op.path,
                parameter.name
            )));
        }
        let name = argument(&parameter.name);
        let header = format!("{:?}", parameter.name);
        let (ty, value) = if matches!(parameter.schema, Schema::String(_)) {
            ("&str".to_owned(), name.clone())
        } else {
            let ty = types.ty(
                &parameter.schema,
                &format!("{hint}{}", super::type_ident(&parameter.name)),
                "types::",
            );
            if parameter.required {
                (ty, format!("&{name}.to_string()"))
            } else {
                (
                    ty,
                    format!("{name}.map(|value| value.to_string()).as_deref()"),
                )
            }
        };
        if parameter.required {
            arguments.push(format!("{name}: {ty}"));
            chain.push(format!(".header({header}, {value})"));
        } else {
            arguments.push(format!("{name}: Option<{ty}>"));
            chain.push(format!(".optional_header({header}, {value})"));
        }
        let description = parameter
            .description
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map_or_else(String::new, |text| {
                let mut chars = text.chars();
                let first = chars.next().map(|c| c.to_lowercase().collect::<String>());
                format!(": {}{}", first.unwrap_or_default(), chars.as_str())
            });
        header_docs.push(format!(
            "    /// `{name}` is sent as `{}`{description}\n",
            parameter.name
        ));
    }
    let (answer, finish) = match &op.response {
        Response::Empty => ("()".to_owned(), ".empty_answer()"),
        Response::Binary => ("Vec<u8>".to_owned(), ".bytes_answer()"),
        Response::Json(schema) => (
            types.ty(schema, &format!("{hint}Response"), "types::"),
            ".json_answer()",
        ),
    };
    chain.push(finish.to_owned());
    chain.push(".await".to_owned());
    let chain = if chain.concat().len() <= CHAIN_WIDTH {
        chain.concat()
    } else {
        // rustfmt's shape: the root and the first link, then a link a line.
        let mut links = chain.into_iter();
        let mut text: String = links.by_ref().take(2).collect();
        for link in links {
            text.push_str(&format!("\n            {link}"));
        }
        text
    };
    lines.push(path_line(&op.path, &op.path_params));
    lines.push(format!(
        "let call = self.client.call(Method::{}, {}path, {})?;",
        op.method.upper(),
        if op.path_params.is_empty() { "" } else { "&" },
        op.secured
    ));
    lines.push(chain);

    let summary = op.summary.as_deref();
    let fallback = format!("`{} {}`.", op.method.upper(), op.path);
    let mut text = doc(summary, &fallback, "    ");
    if op.optional_sign_in {
        text.push_str("    ///\n    /// Sends the session's token when there is one.\n");
    } else if op.secured {
        text.push_str("    ///\n    /// Sends the session's token.\n");
    }
    for line in header_docs {
        text.push_str("    ///\n");
        text.push_str(&line);
    }
    let name = call_name(named);
    let page_type = answer.clone();
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
    if let (Some(paging), Some(query_type), true) = (
        types.document().paging(op),
        &query_type,
        op.header_params.is_empty(),
    ) {
        text.push('\n');
        text.push_str(&pages(
            named, paging, types, &arguments, query_type, &page_type,
        ));
    }
    let typed = arguments
        .iter()
        .any(|argument| argument.contains("types::"))
        || answer.contains("types::");
    Ok((text, typed))
}

/// The integer type of the property `name` of the page schema `schema`
/// (`i32` or `i64`).
fn integer_of(types: &mut Types<'_>, schema: &Schema, name: &str) -> String {
    let schema = match schema {
        Schema::Ref(component) => types.document().schemas.get(component).cloned(),
        other => Some(other.clone()),
    };
    let property = match schema {
        Some(Schema::Object { properties, .. }) => properties
            .into_iter()
            .find(|property| property.name == name)
            .map(|property| property.schema),
        _ => None,
    };
    property.map_or_else(|| "i64".to_owned(), |schema| types.ty(&schema, "", ""))
}

/// `expr`, of the integer type `ty`, as an `i64`.
fn wide(expr: &str, ty: &str) -> String {
    if ty == "i64" {
        expr.to_owned()
    } else {
        format!("i64::from({expr})")
    }
}

/// `<call>_pages`: the call's pages, one at a time, from the query's page
/// (or cursor) on. `arguments` are the call's, `&self` first.
fn pages(
    named: &Named<'_>,
    paging: Paging,
    types: &mut Types<'_>,
    arguments: &[String],
    query_type: &str,
    page_type: &str,
) -> String {
    let op = named.op;
    let call = call_name(named);
    let query = format!("types::{query_type}");
    // Each path parameter the call takes by reference is owned by the
    // pages, and lent to each page's call.
    let mut owned = Vec::new();
    let mut passed = Vec::new();
    for parameter in &op.path_params {
        let name = argument(&parameter.name);
        if matches!(parameter.schema, Schema::String(_)) {
            owned.push(name.clone());
            passed.push(format!("&{name}"));
        } else {
            passed.push(name);
        }
    }
    passed.push("&query".to_owned());
    let (next, start) = match paging {
        Paging::Numbered => {
            let Response::Json(schema) = &op.response else {
                unreachable!("a paged list answers JSON")
            };
            let page = integer_of(types, schema, "page");
            let per_page = integer_of(types, schema, "per_page");
            let total = integer_of(types, schema, "total");
            let asked = op
                .query_params
                .iter()
                .find(|parameter| parameter.name == "page")
                .map_or_else(|| "i64".to_owned(), |p| types.ty(&p.schema, "", ""));
            let number = match (page.as_str(), asked.as_str()) {
                (from, to) if from == to => "Some(page.page + 1)".to_owned(),
                (_, "i64") => "Some(i64::from(page.page) + 1)".to_owned(),
                (_, to) => format!("{to}::try_from(page.page + 1).ok()"),
            };
            (
                format!(
                    "let more = {} * {} < {};\n            \
                     let next = more.then(|| {query} {{\n                \
                     page: {number},\n                ..query\n            }});",
                    wide("page.page", &page),
                    wide("page.per_page", &per_page),
                    wide("page.total", &total),
                ),
                "page",
            )
        }
        Paging::Cursor => (
            format!(
                "let next = page.next.clone().map(|cursor| {query} {{\n                \
                 cursor: Some(cursor),\n                ..query\n            }});"
            ),
            "cursor",
        ),
    };
    let read_call = format!("calls.{call}({}).await?", passed.join(", "));
    let body = format!(
        "            let page = {read_call};\n            {next}\n            Ok((page, next))\n"
    );
    let mut text = format!(
        "    /// Every page of [`Self::{call}`] from `query`'s {start} on (the first\n    \
         /// page without one), read one at a time: `while let Some(page) =\n    \
         /// pages.next().await`.\n"
    );
    let result = format!("crate::Pages<'_, {page_type}>");
    let one_line = format!(
        "    pub fn {call}_pages({}) -> {result} {{",
        arguments.join(", ")
    );
    if one_line.len() <= MAX_WIDTH {
        text.push_str(&one_line);
        text.push('\n');
    } else {
        text.push_str(&format!("    pub fn {call}_pages(\n"));
        for argument in arguments {
            text.push_str(&format!("        {argument},\n"));
        }
        text.push_str(&format!("    ) -> {result} {{\n"));
    }
    text.push_str("        let calls = *self;\n");
    for name in &owned {
        text.push_str(&format!("        let {name} = {name}.to_owned();\n"));
    }
    if owned.is_empty() {
        text.push_str(&format!(
            "        let read = move |query: {query}| async move {{\n{body}        }};\n"
        ));
    } else {
        text.push_str(&format!("        let read = move |query: {query}| {{\n"));
        for name in &owned {
            text.push_str(&format!("            let {name} = {name}.clone();\n"));
        }
        let body = body.replace("\n            ", "\n                ");
        text.push_str(&format!(
            "            async move {{\n    {body}            }}\n        }};\n"
        ));
    }
    text.push_str("        crate::Pages::new(query.clone(), read)\n    }\n");
    text
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
