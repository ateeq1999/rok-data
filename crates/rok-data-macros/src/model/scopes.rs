//! The `scopes` attribute: the model's named scopes.
//!
//! A scope is a function from a query chain to a query chain, taking the
//! chain by value: `fn popular(q: Select<User>) -> Select<User> {
//! q.filter(…) }`. The model declares its scopes by path
//! (`#[rok(scopes(popular = path::to::popular))]`) and the derive gives the
//! model an entry (`User::popular()`) and the chain a method
//! (`User::popular().active()`), through a `{Model}Scopes` trait it writes
//! next to the model.
//!
//! A parameterized scope declares its parameters by the same bracketed
//! syntax the entry takes them by (`#[rok(scopes(of_type(ty: i64) =
//! path::to::of_type))]`); the free function takes the query first and
//! the parameters after (`fn of_type(Select<User>, i64) -> Select<User>`),
//! and the entry and the method get the same call shape
//! (`User::of_type(ty)`, `User::popular().of_type(ty)`).

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::parse::Parse;
use syn::spanned::Spanned;
use syn::{Ident, Path, Type};

use crate::naming::{method_ident, unraw};

/// One parameter of a parameterized scope: `ty: i64`.
pub(crate) struct ScopeParam {
    /// The name the parameter is called by (`ty`).
    pub(crate) name: Ident,
    /// The parameter's type (`i64`).
    pub(crate) ty: Type,
}

/// One named scope: the name it is called by, the function behind it,
/// and the parameters a parameterized scope takes (empty otherwise).
pub(crate) struct Scope {
    /// The name the scope is called by (`popular`).
    pub(crate) name: Ident,
    /// The path to the scope function (`path::to::popular`).
    pub(crate) path: Path,
    /// The scope's parameters; an unparameterized scope has none.
    pub(crate) params: Vec<ScopeParam>,
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
    "delete_by_key",
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
    "install_change_notifications",
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
    "for_share",
    "for_update",
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
    // The derive's collection entries.
    "to_collection",
    "to_collection_vec",
];

/// The parsed parameters of a scope: what both the entry and the method
/// take (`ty: i64`) and what they forward to the scope function (`ty`).
struct Params<'a> {
    signature: Vec<&'a ScopeParam>,
    forwarding: Vec<&'a Ident>,
}

impl<'a> Params<'a> {
    /// The parameters of `scope`; an unparameterized scope has none.
    fn of(scope: &'a Scope) -> Self {
        let signature = scope.params.iter().collect();
        let forwarding = scope.params.iter().map(|param| &param.name).collect();
        Self {
            signature,
            forwarding,
        }
    }

    /// Whether the scope takes any parameters.
    fn is_empty(&self) -> bool {
        self.signature.is_empty()
    }
}

/// Parse `scopes(name = path::to::scope, …)` — with an optional parameter
/// list on the name (`name(ty: i64) = path::to::scope`) — into the scopes
/// it declares.
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

        // The parameters, `name(ty: i64)`; absent for the plain form.
        let params = if inner.input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in inner.input);
            content
                .parse_terminated(syn::FnArg::parse, syn::Token![,])?
                .into_iter()
                .map(|arg| match arg {
                    syn::FnArg::Typed(pat) => match *pat.pat {
                        syn::Pat::Ident(ident) => Ok(ScopeParam {
                            name: ident.ident,
                            ty: *pat.ty,
                        }),
                        other => Err(syn::Error::new(
                            other.span(),
                            "a scope parameter is `name: Type`: `name(ty: i64) = path::to::scope`",
                        )),
                    },
                    receiver @ syn::FnArg::Receiver(_) => Err(syn::Error::new(
                        receiver.span(),
                        "a scope parameter is `name: Type`: `name(ty: i64) = path::to::scope`",
                    )),
                })
                .collect::<syn::Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        if !inner.input.peek(syn::Token![=]) {
            return Err(inner.error("expected `name = path::to::scope` or `name(ty: Type) = path::to::scope`"));
        }
        let value: syn::Expr = inner.value()?.parse()?;
        let path = match &value {
            syn::Expr::Path(path) => path.path.clone(),
            other => {
                return Err(syn::Error::new(
                    other.span(),
                    "a scope names a function: `name = path::to::scope` or `name(ty: Type) = path::to::scope`",
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
        out.push(Scope { name, path, params });
        Ok(())
    })
}

/// The model's scope entries: `User::popular()` or `User::of_type(ty)`.
///
/// Each entry starts a fresh chain and hands it to the scope function.
pub(crate) fn entries(model: &Ident, scopes: &[Scope], krate: &TokenStream2) -> Vec<TokenStream2> {
    scopes
        .iter()
        .map(|scope| {
            let name = &scope.name;
            let path = &scope.path;
            let doc = format!("The `{name}` scope.");
            let params = Params::of(scope);
            if params.is_empty() {
                quote! {
                    #[doc = #doc]
                    pub fn #name() -> #krate::Select<#model> {
                        #path(#krate::Select::new())
                    }
                }
            } else {
                let names = params.forwarding;
                let types = params.signature.iter().map(|param| &param.ty);
                quote! {
                    #[doc = #doc]
                    pub fn #name(#(#names: #types),*) -> #krate::Select<#model> {
                        #path(#krate::Select::new(), #(#names),*)
                    }
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

    // The trait's signatures and the impl's bodies, one of each per
    // scope. Neither a trait body nor an impl block separates its
    // members with commas, so the tokens are joined bare. The trait's
    // methods carry the entry's doc: a public trait's items are
    // documented (`deny(missing_docs)` reads generated code too).
    let mut trait_body = TokenStream2::new();
    let mut impl_body = TokenStream2::new();
    for scope in scopes {
        let name = &scope.name;
        let params = Params::of(scope);
        let doc = format!("The `{name}` scope.");
        let signature = if params.is_empty() {
            quote! { fn #name(self) -> #krate::Select<#model>; }
        } else {
            let names = params.forwarding.clone();
            let types: Vec<&Type> = params.signature.iter().map(|param| &param.ty).collect();
            quote! { fn #name(self, #(#names: #types),*) -> #krate::Select<#model>; }
        };
        trait_body.extend(quote! {
            #[doc = #doc]
            #signature
        });

        let path = &scope.path;
        let body = if params.is_empty() {
            quote! {
                fn #name(self) -> #krate::Select<#model> {
                    #path(self)
                }
            }
        } else {
            let names = params.forwarding;
            let types: Vec<&Type> = params.signature.iter().map(|param| &param.ty).collect();
            quote! {
                fn #name(self, #(#names: #types),*) -> #krate::Select<#model> {
                    #path(self, #(#names),*)
                }
            }
        };
        impl_body.extend(body);
    }

    quote! {
        #[doc = #doc]
        pub trait #trait_name {
            #trait_body
        }

        impl #trait_name for #krate::Select<#model> {
            #impl_body
        }
    }
}

/// The shape every scope function must have.
///
/// For an unparameterized scope: `fn(Select<M>) -> Select<M>`, the chain
/// taken by value; for a parameterized one, the declared parameters follow
/// the query (`fn(Select<M>, i64) -> Select<M>`). A scope that is not a function,
/// or that has another shape, fails here with a message that names the
/// attribute.
pub(crate) fn signature_checks(
    model: &Ident,
    scopes: &[Scope],
    krate: &TokenStream2,
) -> Vec<TokenStream2> {
    scopes
        .iter()
        .map(|scope| {
            let path = &scope.path;
            let types = scope.params.iter().map(|param| &param.ty);
            if scope.params.is_empty() {
                quote! {
                    const _: fn(#krate::Select<#model>) -> #krate::Select<#model> = #path;
                }
            } else {
                quote! {
                    const _: fn(#krate::Select<#model>, #(#types),*) -> #krate::Select<#model> = #path;
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod reserved_tests {
    use super::RESERVED;

    /// The runtime's verb names, read from its source: the chain's public
    /// methods (`Select`'s impl blocks) and the model traits' methods. A
    /// scope named like one of them would be unreachable, so each must be
    /// in [`RESERVED`]; this test fails when a new verb is added to the
    /// runtime without it.
    #[test]
    fn every_runtime_verb_is_reserved() {
        let core = concat!(env!("CARGO_MANIFEST_DIR"), "/../rok-data-core/src");
        let read = |path: &str| {
            std::fs::read_to_string(format!("{core}/{path}"))
                .unwrap_or_else(|error| panic!("{path}: {error}"))
        };
        let mut verbs = Vec::new();
        // The chain: `pub fn` / `pub async fn` one level into an impl.
        for path in [
            "query/select.rs",
            "query/select/exec.rs",
            "query/select/chunk.rs",
        ] {
            for line in read(path).lines() {
                let item = line
                    .strip_prefix("    pub async fn ")
                    .or_else(|| line.strip_prefix("    pub fn "));
                verbs.extend(item.map(name));
            }
        }
        // The model traits: every method one level into the trait.
        for path in ["model/row.rs", "model/write.rs"] {
            for line in read(path).lines() {
                let item = line
                    .strip_prefix("    async fn ")
                    .or_else(|| line.strip_prefix("    fn "));
                verbs.extend(item.map(name));
            }
        }
        verbs.retain(|verb| !verb.starts_with("__"));
        assert!(verbs.len() > 40, "the runtime's verbs were read: {verbs:?}");
        let missing: Vec<&String> = verbs
            .iter()
            .filter(|verb| !RESERVED.contains(&verb.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "runtime verbs a scope could shadow, missing from RESERVED: {missing:?}"
        );
    }

    /// The identifier at the start of `item` (`find<'e, E>(` → `find`).
    fn name(item: &str) -> String {
        item.chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect()
    }
}
