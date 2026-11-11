//! The TypeScript Client (workspace ADR 0027): `clients/ts`, emitted from
//! the [`Document`]. Its files:
//!
//! - `src/runtime.ts` — `ApiError` and `FieldIssue` (the web starter's
//!   error shape), `configure` (the base URL, the session's bearer token,
//!   what a rejected token does) and the one `request` every call goes
//!   through. The same text for every document.
//! - `src/types.ts` — a zod schema and its type per component schema
//!   (`ThingSchema`, `type Thing`), checking the server's rules, and each
//!   operation's query parameters as a plain type.
//! - `src/calls.ts` — `api`, one typed call per operation, grouped by tag:
//!   `api.users.update(id, body)`.
//! - `src/queries.ts` — TanStack Query factories: `queries.users.list(q)`
//!   (`queryOptions`) for every `GET`, `mutations.users.update()`
//!   (`mutationOptions`) for every other method, and `keys` to invalidate
//!   by tag.
//! - `src/index.ts` and `README.md`.
//!
//! A call checks a JSON success with its response schema and throws
//! `ContractError` when the body is not what the contract describes;
//! unknown keys are dropped. The Client targets zod 4, which its App
//! depends on.
//!
//! Every file carries the engine's generated marker, so regeneration
//! rewrites it, a hand edit is refused without `--force`, and a file no
//! longer emitted is removed as stale (workspace ADR 0005).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub use super::emit::Error;
use super::emit::{HEADER, MARKDOWN_HEADER};
pub(crate) use super::emit::{Named, header_field, identifier, name_operations};
use super::openapi::Document;

mod calls;
mod queries;
mod types;

/// The runtime every call goes through: the same for every document.
const RUNTIME: &str = include_str!("ts/runtime.ts");

/// The second line of every TypeScript file: the zod major it targets.
const ZOD: &str = "// The Client targets zod 4: its App depends on `zod@^4`.\n";

/// The names `src/runtime.ts`, `src/calls.ts` and `src/queries.ts`
/// export, which `src/index.ts` re-exports beside the types: no type may
/// take one.
const RESERVED: &[&str] = &[
    "ApiError",
    "ContractError",
    "ContractIssue",
    "ResponseSchema",
    "FieldIssue",
    "FieldDetails",
    "ClientConfig",
    "RequestOptions",
    "Call",
    "configure",
    "apiUrl",
    "request",
    "api",
    "keys",
    "queries",
    "mutations",
];

/// The client's files for `document`, each its path relative to the
/// client's directory and its text.
///
/// # Errors
///
/// When a type name the document asks for is one the client's runtime
/// exports, or two of its names collide.
pub fn emit(document: &Document) -> Result<Vec<(PathBuf, String)>, Error> {
    let names = TypeNames::of(document)?;
    let operations = name_operations(&document.operations);
    let mut files = vec![
        (
            PathBuf::from("README.md"),
            format!("{MARKDOWN_HEADER}{}", readme(&operations)),
        ),
        (
            PathBuf::from("src/index.ts"),
            format!(
                "{HEADER}{ZOD}\nexport * from \"./runtime\"\nexport * from \"./types\"\n\
                 export * from \"./calls\"\nexport * from \"./queries\"\n"
            ),
        ),
        (
            PathBuf::from("src/runtime.ts"),
            format!("{HEADER}{ZOD}\n{RUNTIME}"),
        ),
        (
            PathBuf::from("src/types.ts"),
            format!(
                "{HEADER}{ZOD}{}",
                types::file(document, &operations, &names)
            ),
        ),
        (
            PathBuf::from("src/calls.ts"),
            format!("{HEADER}{ZOD}{}", calls::file(&operations, &names)),
        ),
        (
            PathBuf::from("src/queries.ts"),
            format!("{HEADER}{ZOD}{}", queries::file(&operations, &names)),
        ),
    ];
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// A property key: bare when it is an identifier, quoted otherwise.
pub(crate) fn property_key(name: &str) -> String {
    if !name.is_empty()
        && identifier(name) == name
        && !name.starts_with(|c: char| c.is_ascii_digit())
    {
        name.to_owned()
    } else {
        format!("{name:?}")
    }
}

/// A doc comment of `text` at `indent`, or nothing for no text.
pub(crate) fn doc(text: Option<&str>, indent: &str) -> String {
    let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
        return String::new();
    };
    let text = text.replace("*/", "*\\/");
    let lines: Vec<&str> = text.lines().collect();
    if let [line] = lines.as_slice() {
        return format!("{indent}/** {line} */\n");
    }
    let mut out = format!("{indent}/**\n");
    for line in lines {
        if line.trim().is_empty() {
            out.push_str(&format!("{indent} *\n"));
        } else {
            out.push_str(&format!("{indent} * {line}\n"));
        }
    }
    out.push_str(&format!("{indent} */\n"));
    out
}

/// The client's type names: each component schema's, and each operation's
/// query type.
pub(crate) struct TypeNames {
    schemas: BTreeMap<String, String>,
}

impl TypeNames {
    fn of(document: &Document) -> Result<TypeNames, Error> {
        let mut taken: BTreeSet<String> = BTreeSet::new();
        let mut schemas = BTreeMap::new();
        for name in document.schemas.keys() {
            let ident = identifier(name);
            if RESERVED.contains(&ident.as_str()) {
                return Err(Error(format!(
                    "the schema `{name}` takes the name of the client's `{ident}`; rename the \
                     Rust type (or its `#[schema(as = ...)]`)"
                )));
            }
            let constant = types::schema_const(&ident);
            if RESERVED.contains(&constant.as_str()) {
                return Err(Error(format!(
                    "the schema `{name}`'s zod schema would be `{constant}`, the client's own \
                     `{constant}`; rename the Rust type (or its `#[schema(as = ...)]`)"
                )));
            }
            if !taken.insert(ident.clone()) {
                return Err(Error(format!(
                    "two schemas are named `{ident}` in TypeScript; rename one"
                )));
            }
            schemas.insert(name.clone(), ident);
        }
        for named in name_operations(&document.operations) {
            let at = format!("`{} {}`", named.op.method.upper(), named.op.path);
            if !named.op.query_params.is_empty() {
                let query = named.query_type();
                if !taken.insert(query.clone()) {
                    return Err(Error(format!(
                        "the query type of {at} is `{query}`, which a schema is named already"
                    )));
                }
            }
            if named.op.header_params.is_empty() {
                continue;
            }
            let headers = named.headers_type();
            if !taken.insert(headers.clone()) {
                return Err(Error(format!(
                    "the headers type of {at} is `{headers}`, which a type is named already"
                )));
            }
            for parameter in &named.op.header_params {
                let field = header_field(&parameter.name);
                if calls::taken_by_options(&named, &field) {
                    return Err(Error(format!(
                        "{at}: the header `{}` would be the option `{field}`, which the call takes already",
                        parameter.name
                    )));
                }
            }
        }
        Ok(TypeNames { schemas })
    }

    /// The TypeScript name of the component schema `name`.
    pub fn schema(&self, name: &str) -> String {
        self.schemas
            .get(name)
            .cloned()
            .unwrap_or_else(|| identifier(name))
    }
}

/// The client's `README.md`.
fn readme(operations: &[Named<'_>]) -> String {
    let routes = if operations.iter().any(|named| named.op.is_browser_route()) {
        "- `api.<tag>.<operation>Url(...)` for a Browser route (an operation whose \n         every answer is a redirect): its full URL, with its path and query \n         parameters, for a browser to follow; it sends nothing.
"
    } else {
        ""
    };
    let example = operations
        .iter()
        .find(|named| named.is_query() && !named.op.is_browser_route())
        .map_or_else(String::new, |named| {
            format!(
                "\n```ts\nimport {{ api, queries }} from \"<this directory>/src\"\n\n\
                 const page = await api.{group}.{name}(/* ... */)\n\
                 const query = useQuery(queries.{group}.{name}(/* ... */))\n```\n",
                group = named.group,
                name = named.name
            )
        });
    format!(
        "\n# The API's TypeScript client\n\n\
         Generated from the API's `openapi.json` by `rok gen` (and `rok init`); \
         never edit these files by hand — change the API, then `rok gen` re-exports \
         the document and regenerates them.\n\n\
         - `api.<tag>.<operation>(...)`: one typed call per operation, path \
         parameters first, then the body, then the query, then `RequestOptions` \
         with the operation's headers (`{{ ifMatch }}`).\n\
         - `queries.<tag>.<operation>(...)`: `queryOptions` for every `GET`; \
         `mutations.<tag>.<operation>()`: `mutationOptions` for the others; \
         `keys.<tag>`: the prefix of a tag's query keys, to invalidate them.\n\
         - `ApiError` (`status`, `code`, `message`, `details` of `FieldIssue`s on a \
         `422`) is what every failure rejects with.\n\
         - `<Type>Schema`: the zod schema of each type, with the server's rules \
         (lengths, bounds, patterns, formats), for a form to check input with. \
         Every call checks its JSON success with its schema and rejects with \
         `ContractError` (the operation, and each issue's path) when the body is \
         not what the contract describes; unknown keys are dropped.\n\
         - The App depends on zod 4 (`zod@^4`).\n\
         - `configure({{ baseUrl, token, onUnauthorized }})` once, at start-up: the \
         session's bearer token goes to the operations that need it, and a `401` to \
         one of them calls `onUnauthorized`.\n{routes}{example}"
    )
}

#[cfg(test)]
mod tests;
