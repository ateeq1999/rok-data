//! The Rust Client (workspace ADR 0027): `clients/rust`'s sources, emitted
//! from the [`Document`]: an async `reqwest` client with serde types of
//! its own, so a caller (the desktop App, the api App's tests) never
//! compiles with the server. Its files:
//!
//! - `src/runtime.rs` — `Client` (the base URL, the session's bearer
//!   token, what a rejected token does), `ApiError` and `FieldIssue` (the
//!   wire standard's error shape) and the one request every call goes
//!   through. The same text for every document.
//! - `src/types.rs` — a type per component schema, per operation's query,
//!   and per inline schema a type needs a name for.
//! - `src/calls.rs` — one typed `async fn` per operation, grouped by tag:
//!   `client.users().update(id, &body)`.
//! - `src/lib.rs` and `README.md`.
//!
//! The crate's `Cargo.toml` is not emitted: it is the project's (the
//! template scaffolds it), so its dependencies and lints are the caller's
//! to tune. Every emitted file carries the engine's generated marker
//! (workspace ADR 0005).

use std::collections::BTreeMap;
use std::path::PathBuf;

use heck::{ToSnakeCase, ToUpperCamelCase};

pub use super::emit::Error;
use super::emit::{HEADER, MARKDOWN_HEADER, Named, name_operations};
use super::openapi::Document;

mod calls;
mod types;

/// The runtime every call goes through: the same for every document.
const RUNTIME: &str = include_str!("rust/runtime.rs.in");

/// The names a type may not take: the crate root's own exports (the types
/// are re-exported beside them) and the prelude's and serde's names the
/// types module uses.
const RESERVED_TYPES: &[&str] = &[
    "ApiError",
    "Client",
    "FieldIssue",
    "FieldDetails",
    "Pages",
    "Call",
    "Option",
    "Some",
    "None",
    "Result",
    "Ok",
    "Err",
    "String",
    "Vec",
    "Box",
    "Default",
    "Serialize",
    "Deserialize",
];

/// The methods of `Client` itself: no tag may take one.
const CLIENT_METHODS: &[&str] = &[
    "new",
    "with_http",
    "base_url",
    "api_url",
    "token",
    "set_token",
    "on_unauthorized",
    "keep_session",
    "with_header",
    "call",
    "clone",
    "fmt",
];

/// Rust's keywords, strict and reserved: an identifier that is one takes a
/// trailing `_`.
const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
    "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];

/// The client's files for `document`, each its path relative to the
/// client's directory and its text.
///
/// # Errors
///
/// When a type name the document asks for is one the crate root exports
/// or the types module needs, two of its names collide, or a tag's name is
/// one of `Client`'s methods.
pub fn emit(document: &Document) -> Result<Vec<(PathBuf, String)>, Error> {
    let operations = name_operations(&document.operations);
    let mut types = types::Types::new(document)?;
    let calls = calls::file(&operations, &mut types)?;
    let mut files = vec![
        (
            PathBuf::from("README.md"),
            format!("{MARKDOWN_HEADER}{}", readme(&operations)),
        ),
        (PathBuf::from("src/lib.rs"), format!("{HEADER}{LIB}")),
        (
            PathBuf::from("src/runtime.rs"),
            format!("{HEADER}\n{RUNTIME}"),
        ),
        (PathBuf::from("src/calls.rs"), format!("{HEADER}{calls}")),
        (
            PathBuf::from("src/types.rs"),
            format!("{HEADER}{}", types.finish()),
        ),
    ];
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// `src/lib.rs`, after the header.
const LIB: &str = "
//! The API's client, generated from its `openapi.json` by `rok gen`: one
//! typed `async` call per operation, grouped by tag
//! (`client.users().update(id, &body)`), the API's types beside
//! [`Client`], and [`ApiError`] for every failure. See the README.

pub mod calls;
mod runtime;
pub mod types;

pub use runtime::{ApiError, Client, FieldDetails, FieldIssue, Pages};
#[doc(hidden)]
pub use runtime::{Call, nullable, segment};
pub use types::*;

/// The HTTP client underneath, for its header types and its `Client`
/// builder ([`Client::with_http`]).
pub use reqwest;
";

/// The group (tag) of `named`, as a method of `Client`: `api_keys`.
pub(crate) fn group_method(named: &Named<'_>) -> String {
    ident(&named.group.to_snake_case())
}

/// The group (tag) of `named`, as the type of its calls: `ApiKeys`.
pub(crate) fn group_type(named: &Named<'_>) -> String {
    type_ident(&named.group)
}

/// The name of `named`'s call in its group: `get_one`.
pub(crate) fn call_name(named: &Named<'_>) -> String {
    ident(&named.name.to_snake_case())
}

/// `name` as a Rust identifier in `snake_case`: characters an identifier
/// cannot hold become `_`, a leading digit gets one before it, a keyword
/// one after it.
pub(crate) fn ident(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if KEYWORDS.contains(&out.as_str()) {
        out.push('_');
    }
    out
}

/// `name` as a Rust type name in `UpperCamelCase`.
pub(crate) fn type_ident(name: &str) -> String {
    let camel = name.to_upper_camel_case();
    let mut out: String = camel.chars().filter(char::is_ascii_alphanumeric).collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'T');
    }
    if out == "Self" {
        out.push('_');
    }
    out
}

/// A doc comment of `text` at `indent`, or of `fallback` when there is no
/// text.
pub(crate) fn doc(text: Option<&str>, fallback: &str, indent: &str) -> String {
    let text = text
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(fallback);
    let mut out = String::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            out.push_str(&format!("{indent}///\n"));
        } else {
            out.push_str(&format!("{indent}/// {line}\n"));
        }
    }
    out
}

/// The tags' method names, each once, refusing one `Client` has already.
pub(crate) fn check_groups(operations: &[Named<'_>]) -> Result<(), Error> {
    let mut seen = BTreeMap::new();
    for named in operations {
        let method = group_method(named);
        if CLIENT_METHODS.contains(&method.as_str()) {
            return Err(Error(format!(
                "the tag `{}` takes the name of the client's `{method}`; rename the tag",
                named.op.tag.as_deref().unwrap_or("default")
            )));
        }
        match seen.get(&method) {
            Some(group) if *group != named.group => {
                return Err(Error(format!(
                    "two tags are named `{method}` in Rust; rename one"
                )));
            }
            _ => {
                seen.insert(method, named.group.clone());
            }
        }
    }
    Ok(())
}

/// The client's `README.md`.
fn readme(operations: &[Named<'_>]) -> String {
    let example = operations
        .iter()
        .find(|named| named.is_query() && named.op.path_params.is_empty())
        .map_or_else(String::new, |named| {
            let query = if named.op.query_params.is_empty() {
                String::new()
            } else {
                format!("&{}::default()", type_ident(&named.query_type()))
            };
            format!(
                "\n```rust\nlet client = Client::new(\"http://127.0.0.1:8080\");\n\
                 let answer = client.{}().{}({query}).await?;\n```\n",
                group_method(named),
                call_name(named),
            )
        });
    format!(
        "\n# The API's Rust client\n\n\
         Generated from the API's `openapi.json` by `rok gen` (and `rok init`); \
         never edit its `src/` by hand — change the API, then `rok gen` re-exports \
         the document and regenerates them. `Cargo.toml` is yours: the generated \
         code needs `reqwest` (with `json`), `serde` (with `derive`) and `serde_json`.\n\n\
         - `client.<tag>().<operation>(...)`: one typed `async` call per operation, \
         path parameters first, then the body, then the query, then the headers \
         (`if_match`).\n\
         - A list that pages (by page number or by cursor) also has \
         `client.<tag>().<operation>_pages(&query)`, its `Pages`: `while let Some(page) \
         = pages.next().await` reads one page at a time, to the last.\n\
         - The API's types (serde) sit beside `Client`; a property that may be left \
         out is an `Option`, one that may also be `null` an `Option<Option<_>>`.\n\
         - `ApiError` (`status`, `code`, `message`, `details` of `FieldIssue`s on a \
         `422`) is what every failure returns.\n\
         - `Client::new(base_url)`, then `set_token(Some(token))` after signing in: \
         the session's bearer token goes to the operations that need it, and a `401` \
         to one of them calls the `on_unauthorized` handler (unless the call is made \
         through `keep_session()`). Clones share the session.\n{example}"
    )
}

#[cfg(test)]
mod tests;
