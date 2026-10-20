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
