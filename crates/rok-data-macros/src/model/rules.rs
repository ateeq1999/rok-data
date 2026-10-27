//! The field rules and transforms both derives check: `#[rok(validate(…))]`
//! and `#[rok(transform(…))]`, turned into the statements of a
//! `validate` / `__check` body and a `__transform` body.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Ident, Path};

use super::{Field, Rule};
use crate::naming::unraw;

/// One `#[rok(transform(…))]` step.
pub(crate) enum Transform {
    /// `trim`, `lowercase`, `uppercase`, `normalize_email`.
    Builtin(Ident),
    /// `custom = path`, a `fn(T) -> T`.
    Custom(Path),
}

/// The built-in transforms' names.
const BUILTIN_TRANSFORMS: &[&str] = &["trim", "lowercase", "uppercase", "normalize_email"];

/// Parses `transform(trim, lowercase, custom = path)` into `out`, in order.
pub(crate) fn parse_transforms(
    meta: &syn::meta::ParseNestedMeta<'_>,
    out: &mut Vec<Transform>,
) -> syn::Result<()> {
    meta.parse_nested_meta(|inner| {
        if inner.path.is_ident("custom") {
            out.push(Transform::Custom(inner.value()?.parse()?));
            return Ok(());
        }
        match inner.path.get_ident() {
            Some(name) if BUILTIN_TRANSFORMS.contains(&name.to_string().as_str()) => {
                out.push(Transform::Builtin(name.clone()));
                Ok(())
            }
            _ => Err(inner.error(
                "unknown transform; expected `trim`, `lowercase`, `uppercase`, `normalize_email` or `custom = path`",
            )),
        }
    })
}

/// The statements that add every failed field rule of `fields` to a
/// `ValidationErrors` named `errors`, reading the values from `self`.
pub(crate) fn field_checks(fields: &[&Field], krate: &TokenStream2) -> Vec<TokenStream2> {
    fields
        .iter()
        .flat_map(|f| {
            let ident = &f.ident;
            let field = unraw(ident);
            let v = quote!(#krate::validate);
            f.rules.iter().map(move |rule| {
                let opt = |e: &Option<syn::Expr>, cast: TokenStream2| match e {
                    Some(e) => quote!(::core::option::Option::Some((#e) as #cast)),
                    None => quote!(::core::option::Option::None),
                };
                let (code, check) = match rule {
                    Rule::Length(min, max) => {
                        let (min, max) = (opt(min, quote!(usize)), opt(max, quote!(usize)));
                        ("length", quote!(#v::length(&self.#ident, #min, #max)))
                    }
                    Rule::Range(min, max) => {
                        let (min, max) = (opt(min, quote!(f64)), opt(max, quote!(f64)));
                        ("range", quote!(#v::range(&self.#ident, #min, #max)))
                    }
                    Rule::Email => ("email", quote!(#v::email(&self.#ident))),
                    Rule::NonEmpty => ("non_empty", quote!(#v::non_empty(&self.#ident))),
                    Rule::Custom(path) => ("custom", quote!(#path(&self.#ident))),
                };
                quote! {
                    if let ::core::result::Result::Err(message) = #check {
                        errors.add(#field, #code, message);
                    }
                }
            })
        })
        .collect()
}

/// The statements that run every field's transforms on `self`, in order.
pub(crate) fn transform_steps(fields: &[&Field], krate: &TokenStream2) -> Vec<TokenStream2> {
    fields
        .iter()
        .flat_map(|f| {
            let ident = &f.ident;
            f.transforms.iter().map(move |transform| match transform {
                Transform::Builtin(name) => {
                    quote!(self.#ident = #krate::validate::transform::#name(self.#ident);)
                }
                Transform::Custom(path) => quote!(self.#ident = #path(self.#ident);),
            })
        })
        .collect()
}
