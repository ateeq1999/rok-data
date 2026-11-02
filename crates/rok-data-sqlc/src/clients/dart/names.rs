//! Dart names and literals: identifiers that are not keywords, string
//! literals that interpolate nothing, doc comments.

use heck::{ToLowerCamelCase, ToUpperCamelCase};

/// Dart's reserved words and the built-in identifiers a declaration
/// cannot take.
const KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "assert",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "covariant",
    "default",
    "deferred",
    "do",
    "dynamic",
    "else",
    "enum",
    "export",
    "extends",
    "extension",
    "external",
    "factory",
    "false",
    "final",
    "finally",
    "for",
    "Function",
    "get",
    "if",
    "implements",
    "import",
    "in",
    "interface",
    "is",
    "late",
    "library",
    "mixin",
    "new",
    "null",
    "operator",
    "part",
    "required",
    "rethrow",
    "return",
    "set",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typedef",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// The members every generated class has, which a field cannot take.
const MEMBERS: &[&str] = &[
    "fromJson",
    "toJson",
    "hashCode",
    "runtimeType",
    "toString",
    "noSuchMethod",
];

/// The members every generated enum has besides, which a value cannot
/// take.
const ENUM_MEMBERS: &[&str] = &["values", "index", "name", "value"];

/// The `dart:core` types the generated code names, and the client's own
/// names, which no model may shadow.
const RESERVED_TYPES: &[&str] = &[
    "String",
    "int",
    "double",
    "num",
    "bool",
    "List",
    "Map",
    "MapEntry",
    "Object",
    "Null",
    "Future",
    "FutureOr",
    "Uri",
    "Duration",
    "Exception",
    "FormatException",
    "Enum",
    "Record",
    "Type",
    "ApiClient",
    "ApiException",
    "FieldIssue",
    "Session",
    "RequestOptions",
    "Bytes",
    "Nullable",
    "Call",
    "Transport",
];

/// Whether a model may not take the type name `name`.
pub(crate) fn is_reserved_type(name: &str) -> bool {
    RESERVED_TYPES.contains(&name)
}

/// `text` with every character an identifier cannot hold dropped, and a
/// leading digit prefixed.
fn sanitize(text: &str, prefix: &str) -> String {
    let out: String = text
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$')
        .collect();
    let out = out.trim_start_matches('_').to_owned();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("{prefix}{out}")
    } else {
        out
    }
}

/// A member name: lowerCamelCase, a keyword or a generated member taking
/// a trailing `_`.
pub(crate) fn member_name(name: &str) -> String {
    let ident = sanitize(&name.to_lower_camel_case(), "n");
    if KEYWORDS.contains(&ident.as_str()) || MEMBERS.contains(&ident.as_str()) {
        format!("{ident}_")
    } else {
        ident
    }
}

/// A field's name, for the JSON key `name`.
pub(crate) fn field_name(name: &str) -> String {
    member_name(name)
}

/// An enum value's name, for the wire value `value`.
pub(crate) fn enum_value(value: &str) -> String {
    let ident = member_name(value);
    if ENUM_MEMBERS.contains(&ident.as_str()) {
        format!("{ident}_")
    } else {
        ident
    }
}

/// A class name: UpperCamelCase (`PaginatedResponse_UserResponse` is
/// `PaginatedResponseUserResponse`).
pub(crate) fn class_name(name: &str) -> String {
    sanitize(&name.to_upper_camel_case(), "Schema")
}

/// A Dart string literal of `text`, interpolating nothing.
pub(crate) fn string(text: &str) -> String {
    let mut out = String::from("'");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '$' => out.push_str("\\$"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// A `///` doc comment of `text` at `indent`, or nothing for no text.
pub(crate) fn doc(text: Option<&str>, indent: &str) -> String {
    let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
        return String::new();
    };
    text.lines()
        .map(|line| {
            let line = line.trim_end();
            if line.is_empty() {
                format!("{indent}///\n")
            } else {
                format!("{indent}/// {line}\n")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_dart_identifiers() {
        assert_eq!(field_name("avatar_url"), "avatarUrl");
        assert_eq!(field_name("default"), "default_");
        assert_eq!(field_name("toJson"), "toJson_");
        assert_eq!(field_name("name"), "name");
        assert_eq!(enum_value("name"), "name_");
        assert_eq!(enum_value("created_at"), "createdAt");
        assert_eq!(enum_value("2fa"), "n2fa");
        assert_eq!(
            class_name("PaginatedResponse_UserResponse"),
            "PaginatedResponseUserResponse"
        );
        assert_eq!(string("it's $5"), "'it\\'s \\$5'");
    }
}
