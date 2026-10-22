//! The `scopes` attribute: the model's named scopes.
//!
//! A scope is a function from a query chain to a query chain. The model
//! declares its scopes by path (`#[rok(scopes(popular = path::to::popular))]`)
//! and the derive gives the model an entry (`User::popular()`) and the
//! chain a method (`User::popular().active()`), through the runtime's
//! `Scope` trait.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Ident, Path};

use crate::naming::{method_ident, unraw};

/// One named scope: the name it is called by, and the function behind it.
pub(crate) struct Scope {
    /// The name the scope is called by (`popular`).
    pub(crate) name: Ident,
    /// The path to the scope function (`path::to::popular`).
    pub(crate) path: Path,
}

/// The names the runtime already uses.
///
/// A scope called `count` would put an inherent `User::count()` where the
/// model's `Model::count` lives, and a chain method `count` where the
/// chain's own `count` lives — the scope would be unreachable on the
/// chain. The derive refuses those names rather than shipping a scope
/// that cannot be called.
const RESERVED: &[&str] = &[
    // The `Model` trait's verbs.
    "all",
    "changes",
    "copy_in",
    "count",
    "default_scope",
    "delete",
    "exists",
    "filter",
    "find",
    "find_many",
    "find_or_fail",
    "force_delete",
    "from_row_at",
    "insert",
    "insert_all",
    "insert_builder",
    "insert_many",
    "is_trashed",
    "key_filter",
    "key_values",
    "order_by",
    "primary_key",
    "primary_key_column",
    "query",
    "reload",
    "restore",
    "track",
    "uninstall_change_notifications",
    "update",
    "update_all",
    "update_only",
    "upsert",
    "upsert_on",
    "validate",
    "value_of",
    "values",
    "where_",
    // The chain's verbs.
    "all_tenants",
    "avg",
    "chunk",
    "cursor_paginate",
    "delete_sql",
    "filter_if",
    "filter_opt",
    "first",
    "get",
    "group_by",
    "having",
    "limit",
    "max",
    "memoize",
    "min",
    "new",
    "offset",
    "on_primary",
    "one",
    "only_trashed",
    "paginate",
    "scope",
    "select",
    "stream",
    "sum",
    "to_sql",
    "unscoped",
    "with",
    "with_trashed",
];

/// Parse `scopes(name = path::to::scope, …)` into the scopes it declares.
pub(crate) fn parse_scopes(
    meta: &syn::meta::ParseNestedMeta<'_>,
    out: &mut Vec<Scope>,
) -> syn::Result<()> {
    meta.parse_nested_meta(|inner| {
        let name = inner
            .path
            .get_ident()
            .cloned()
            .ok_or_else(|| inner.error("expected `name = path::to::scope`"))?;
        if !inner.input.peek(syn::Token![=]) {
            return Err(inner.error("expected `name = path::to::scope`"));
        }
        let value: syn::Expr = inner.value()?.parse()?;
        let path = match &value {
            syn::Expr::Path(path) => path.path.clone(),
            other => {
                return Err(syn::Error::new(
                    other.span(),
                    "a scope names a function: `name = path::to::scope`",
                ));
            }
        };
        let name = method_ident(&name)?;
        let plain = unraw(&name);
        if RESERVED.contains(&plain.as_str()) {
            return Err(syn::Error::new(
                name.span(),
                format!("`{plain}` is a verb of the runtime; name the scope differently"),
            ));
        }
        if out.iter().any(|scope: &Scope| scope.name == name) {
            return Err(syn::Error::new(
                name.span(),
                format!("the scope `{plain}` is declared twice"),
            ));
        }
        out.push(Scope { name, path });
        Ok(())
    })
}

/// The model's scope entries: `User::popular()`.
///
/// Each entry starts a fresh chain and hands it to the scope function.
pub(crate) fn entries(model: &Ident, scopes: &[Scope], krate: &TokenStream2) -> Vec<TokenStream2> {
    scopes
        .iter()
        .map(|scope| {
            let name = &scope.name;
            let path = &scope.path;
            let doc = format!("The `{name}` scope.");
            quote! {
                #[doc = #doc]
                pub fn #name() -> #krate::Select<#model> {
                    #krate::Scope::scope(#krate::Select::new(), #path)
                }
            }
        })
        .collect()
}

/// The trait the chain's scope methods hang off: `UserScopes`.
///
/// The trait is written next to the model, so a call site reaches it as
/// `models::UserScopes` and brings it into scope with a `use`.
pub(crate) fn trait_declaration(
    model: &Ident,
    scopes: &[Scope],
    krate: &TokenStream2,
) -> TokenStream2 {
    let trait_name = format_ident!("{}Scopes", model);
    let doc = format!("The named scopes of `{model}`.");
    let pairs = scopes.iter().map(|scope| {
        let name = &scope.name;
        let path = &scope.path;
        quote!(#name => #path)
    });
    quote! {
        #krate::declare_scopes! {
            #[doc = #doc]
            pub trait #trait_name for #model {
                #(#pairs,)*
            }
        }
    }
}

/// The shape every scope function must have: `fn(&Select<M>) -> Select<M>`.
///
/// A scope that is not a function, or that has another shape, fails here
/// with a message that names the attribute.
pub(crate) fn signature_checks(
    model: &Ident,
    scopes: &[Scope],
    krate: &TokenStream2,
) -> Vec<TokenStream2> {
    scopes
        .iter()
        .map(|scope| {
            let path = &scope.path;
            quote! {
                const _: fn(&#krate::Select<#model>) -> #krate::Select<#model> = #path;
            }
        })
        .collect()
}
