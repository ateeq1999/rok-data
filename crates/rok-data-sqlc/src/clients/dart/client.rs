//! `lib/src/client.dart`: `ApiClient`, with a getter per tag whose class
//! holds one typed method per operation: `api.users.update(id, body)`.
//! A method takes its path parameters first, in the path's order, then
//! its body, positionally; then its query parameters by name (required
//! when the document says so), then `options`.

use heck::ToUpperCamelCase;

use super::super::openapi::{Body, Response};
use super::super::ts::Named;
use super::Error;
use super::models::Models;
use super::names::{doc, member_name, string};
use super::ty::Ty;

/// The class of a tag's operations: `users` is `UsersApi`.
pub(super) fn group_class(group: &str) -> String {
    format!("{}Api", group.to_upper_camel_case())
}

/// The text of `lib/src/client.dart` (after the header). Inline schemas
/// of bodies and responses are declared in `models`.
pub(super) fn file(operations: &[Named<'_>], models: &mut Models<'_>) -> Result<String, Error> {
    let mut groups: Vec<(&str, Vec<String>)> = Vec::new();
    let mut uses_models = false;
    for named in operations {
        let (text, named_models) = method(named, models)?;
        uses_models |= named_models;
        match groups.iter_mut().find(|(group, _)| *group == named.group) {
            Some((_, methods)) => methods.push(text),
            None => groups.push((&named.group, vec![text])),
        }
    }
    let mut out = String::from("\nimport 'package:http/http.dart' as http;\n\n");
    if uses_models {
        out.push_str("import 'models.dart';\n");
    }
    out.push_str("import 'runtime.dart';\n\n");
    out.push_str(
        "/// The API's client: one typed method per operation, grouped by tag.\n\
         final class ApiClient {\n  \
         /// A client of the API at `baseUrl` (its origin, such as\n  \
         /// `https://api.example.com`). `session` gives the bearer token the\n  \
         /// secured operations send, and hears when the API rejects it;\n  \
         /// `httpClient` replaces the default HTTP client (and is not closed\n  \
         /// by [close]).\n  \
         ApiClient({\n    required String baseUrl,\n    Session? session,\n    \
         http.Client? httpClient,\n  }) : _transport = Transport(\n         \
         baseUrl: baseUrl,\n         session: session,\n         \
         httpClient: httpClient,\n       );\n\n  \
         final Transport _transport;\n",
    );
    for (group, _) in &groups {
        out.push_str(&format!(
            "\n  /// The `{group}` operations.\n  {} get {} => {}._(_transport);\n",
            group_class(group),
            group_getter(group),
            group_class(group)
        ));
    }
    out.push_str(
        "\n  /// `path` on the API's origin: an image's URL, say.\n  \
         Uri url(String path) => _transport.url(path);\n\n  \
         /// Closes the HTTP client, unless it was handed in.\n  \
         void close() => _transport.close();\n}\n",
    );
    for (group, methods) in &groups {
        let class = group_class(group);
        out.push_str(&format!(
            "\n/// The `{group}` operations.\nfinal class {class} {{\n  \
             const {class}._(this._transport);\n\n  final Transport _transport;\n"
        ));
        for text in methods {
            out.push('\n');
            out.push_str(text);
        }
        out.push_str("}\n");
    }
    Ok(out)
}

/// A tag's getter on `ApiClient`: its name, unless `ApiClient` has a
/// member of that name.
fn group_getter(group: &str) -> String {
    let name = member_name(group);
    if matches!(name.as_str(), "url" | "close") {
        format!("{name}_")
    } else {
        name
    }
}

/// A parameter name not yet in `taken`.
fn unique(name: &str, taken: &mut Vec<String>) -> String {
    let mut name = member_name(name);
    while taken.contains(&name) {
        name.push('_');
    }
    taken.push(name.clone());
    name
}

/// One method, and whether it names a model.
fn method(named: &Named<'_>, models: &mut Models<'_>) -> Result<(String, bool), Error> {
    let op = named.op;
    let at = format!("`{} {}`", op.method.upper(), op.path);
    let context = format!(
        "{}{}",
        named.group.to_upper_camel_case(),
        named.name.to_upper_camel_case()
    );
    let mut taken = vec!["options".to_owned()];
    let mut positional = Vec::new();
    let mut named_params = Vec::new();
    let mut uses_models = false;
    let mut path = op.path.clone();
    for parameter in &op.path_params {
        let here = format!("{at}, path parameter `{}`", parameter.name);
        let ty = models.ty(&parameter.schema, &context, false, &here)?;
        if !ty.is_scalar() || ty.is_nullable() {
            return Err(unsupported(&here, "a path parameter that is not a scalar"));
        }
        let name = unique(&parameter.name, &mut taken);
        uses_models |= ty.names_a_model();
        positional.push(format!("{} {name}", ty.dart()));
        let text = match ty {
            Ty::String => name.clone(),
            Ty::Enum(_) => format!("{name}.toJson()"),
            _ => format!("{name}.toString()"),
        };
        path = path.replace(
            &format!("{{{}}}", parameter.name),
            &format!("\u{0}{{Uri.encodeComponent({text})}}"),
        );
    }
    let mut call_fields = Vec::new();
    match &op.body {
        Some(Body::Json(schema)) => {
            let ty = models.ty(schema, &format!("{context}Body"), true, &at)?;
            let name = unique("body", &mut taken);
            uses_models |= ty.names_a_model();
            positional.push(format!("{} {name}", ty.dart()));
            call_fields.push(format!("json: {}", ty.encode(&name, 0)));
        }
        Some(Body::Binary) => {
            let name = unique("body", &mut taken);
            positional.push(format!("Bytes {name}"));
            call_fields.push(format!("bytes: {name}"));
        }
        None => {}
    }
    let mut query = Vec::new();
    for parameter in &op.query_params {
        let here = format!("{at}, query parameter `{}`", parameter.name);
        let context = format!("{context}{}", parameter.name.to_upper_camel_case());
        let ty = models.ty(&parameter.schema, &context, false, &here)?;
        if !ty.is_scalar() {
            return Err(unsupported(&here, "a query parameter that is not a scalar"));
        }
        let ty = if parameter.required {
            ty
        } else {
            ty.nullable()
        };
        let name = unique(&parameter.name, &mut taken);
        uses_models |= ty.names_a_model();
        named_params.push(format!(
            "{}{} {name}",
            if parameter.required { "required " } else { "" },
            ty.dart()
        ));
        query.push(format!(
            "{}: {}",
            string(&parameter.name),
            ty.encode(&name, 0)
        ));
    }
    named_params.push("RequestOptions? options".to_owned());
    if !query.is_empty() {
        call_fields.insert(0, format!("query: {{{}}}", query.join(", ")));
    }
    if op.secured {
        call_fields.push("secured: true".to_owned());
    }
    // The path's literal text escaped, then each parameter interpolated.
    let path = string(&path).replace('\u{0}', "$");
    let mut call = format!("Call(\n      {},\n      {path}", string(op.method.upper()));
    for field in &call_fields {
        call.push_str(&format!(",\n      {field}"));
    }
    call.push_str(",\n    )");
    let (result, send) = match &op.response {
        Response::Empty => (
            "void".to_owned(),
            format!("_transport.none(\n    {call},\n    options,\n  )"),
        ),
        Response::Binary => (
            "Bytes".to_owned(),
            format!("_transport.bytes(\n    {call},\n    options,\n  )"),
        ),
        Response::Json(schema) => {
            let ty = models.ty(schema, &format!("{context}Response"), false, &at)?;
            uses_models |= ty.names_a_model();
            (
                ty.dart(),
                format!(
                    "_transport.json<{}>(\n    {call},\n    (json) => {},\n    options,\n  )",
                    ty.dart(),
                    ty.decode("json", 0)
                ),
            )
        }
    };
    let summary = op
        .summary
        .clone()
        .unwrap_or_else(|| format!("`{} {}`.", op.method.upper(), op.path));
    // `dart format`'s shape: positional parameters, the last one opening
    // the named ones' braces.
    let mut parameters = String::new();
    for (index, parameter) in positional.iter().enumerate() {
        let last = index + 1 == positional.len();
        parameters.push_str(&format!(
            "    {parameter},{}\n",
            if last { " {" } else { "" }
        ));
    }
    let open = if positional.is_empty() { "{" } else { "" };
    for parameter in &named_params {
        parameters.push_str(&format!("    {parameter},\n"));
    }
    let text = format!(
        "{}  Future<{result}> {}({open}\n{parameters}  }}) => {send};\n",
        doc(Some(&summary), "  "),
        member_name(&named.name),
    );
    Ok((text, uses_models))
}

/// `construct`, at `at`, is outside what the Dart client reads.
fn unsupported(at: &str, construct: &str) -> Error {
    Error(format!(
        "{at}: {construct} is not supported by the Dart client"
    ))
}
