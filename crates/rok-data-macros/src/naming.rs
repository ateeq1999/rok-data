//! The naming helpers the derives share.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::Ident;

pub(crate) fn rename(name: &str, rule: &str) -> syn::Result<String> {
    let snake = snake_case(name);
    Ok(match rule {
        "snake_case" => snake,
        "SCREAMING_SNAKE_CASE" => snake.to_uppercase(),
        "kebab-case" => snake.replace('_', "-"),
        "lowercase" => name.to_lowercase(),
        "UPPERCASE" => name.to_uppercase(),
        "PascalCase" => name.to_owned(),
        _ => {
            return Err(syn::Error::new(
                Span::call_site(),
                "`rename_all` must be one of snake_case, SCREAMING_SNAKE_CASE, kebab-case, lowercase, UPPERCASE, PascalCase",
            ));
        }
    })
}

/// The Rust keywords a generated name may carry. A generated item whose
/// name is one of them is emitted as a raw identifier (`r#type`), so the
/// generated code compiles.
const RESERVED: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let",
    "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return",
    "static", "struct", "trait", "true", "try", "type", "typeof", "unsafe", "unsized", "use",
    "virtual", "where", "while", "yield",
];

/// The keywords that have no raw form: `r#crate`, `r#self`, `r#Self` and
/// `r#super` do not exist, so a relation named after one of them is
/// refused instead of emitted.
const NOT_RAW: &[&str] = &["crate", "self", "Self", "super"];

/// The ident of a relation named `name`: the name itself, or a raw
/// identifier when the name is a Rust keyword (`r#type`).
///
/// A keyword with no raw form is an error: the generated accessor would
/// not compile.
pub(crate) fn relation_ident(name: &str, span: Span) -> syn::Result<Ident> {
    if NOT_RAW.contains(&name) {
        return Err(no_raw_form(name, span));
    }
    if RESERVED.contains(&name) {
        return Ok(Ident::new_raw(name, span));
    }
    Ok(Ident::new(name, span))
}

/// The name a generated accessor carries: the name as it stands, or a raw
/// identifier when the name is a Rust keyword (`r#type`), so the
/// generated code compiles.
pub(crate) fn method_ident(name: &Ident) -> syn::Result<Ident> {
    let plain = unraw(name);
    if NOT_RAW.contains(&plain.as_str()) {
        return Err(no_raw_form(&plain, name.span()));
    }
    if RESERVED.contains(&plain.as_str()) {
        return Ok(Ident::new_raw(&plain, name.span()));
    }
    Ok(name.clone())
}

/// The error for a keyword that has no raw form: `r#self` does not exist.
fn no_raw_form(name: &str, span: Span) -> syn::Error {
    syn::Error::new(
        span,
        format!("`{name}` is a Rust keyword with no raw form; name the relation differently"),
    )
}

pub(crate) fn crate_path() -> TokenStream2 {
    let found = crate_name("rok-data")
        .map(|c| (c, "rok_data"))
        .or_else(|_| crate_name("rok-data-core").map(|c| (c, "rok_data_core")));
    match found {
        // Inside rok-data's own tests/examples the crate is reachable by name.
        Ok((FoundCrate::Itself, default)) => {
            let ident = Ident::new(default, Span::call_site());
            quote!(::#ident)
        }
        Ok((FoundCrate::Name(name), _)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
        Err(_) => quote!(::rok_data),
    }
}

pub(crate) fn unraw(ident: &Ident) -> String {
    let s = ident.to_string();
    s.strip_prefix("r#").map(str::to_owned).unwrap_or(s)
}

pub(crate) fn snake_case(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let prev_lower =
                i > 0 && (chars[i - 1].is_lowercase() || chars[i - 1].is_ascii_digit());
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            let prev_upper = i > 0 && chars[i - 1].is_uppercase();
            if prev_lower || (prev_upper && next_lower) {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn pluralize(s: &str) -> String {
    let ends_with_consonant_y =
        s.ends_with('y') && !s.chars().rev().nth(1).is_some_and(|c| "aeiou".contains(c));
    if ends_with_consonant_y {
        format!("{}ies", &s[..s.len() - 1])
    } else if ["s", "x", "z", "ch", "sh"].iter().any(|e| s.ends_with(e)) {
        format!("{s}es")
    } else {
        format!("{s}s")
    }
}
