//! The `FromRow` derive's expansion.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, Ident, LitStr, Path, spanned::Spanned};

use super::Field;
use super::expand::parse_fields;
use crate::naming::crate_path;
pub(crate) fn from_row_tokens(
    name: &Ident,
    fields: &[Field],
    krate: &TokenStream2,
) -> TokenStream2 {
    let inits = fields.iter().map(|f| {
        let ident = &f.ident;
        let column = &f.column;
        if f.skip {
            quote!(#ident: ::core::default::Default::default())
        } else {
            quote!(#ident: #krate::__private::Row::try_get(row, #column)?)
        }
    });
    quote! {
        impl<'r> #krate::__private::FromRow<'r, #krate::__private::PgRow> for #name {
            fn from_row(
                row: &'r #krate::__private::PgRow,
            ) -> ::core::result::Result<Self, #krate::__private::SqlxError> {
                ::core::result::Result::Ok(Self { #(#inits,)* })
            }
        }
    }
}

pub(crate) fn expand_from_row(input: DeriveInput) -> syn::Result<TokenStream2> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.generics.span(),
            "`#[derive(FromRow)]` does not support generic structs",
        ));
    }
    let mut krate: Option<Path> = None;
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("rok")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                krate = Some(meta.value()?.parse::<LitStr>()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("unknown `rok` attribute for `FromRow`; expected `crate`"))
            }
        })?;
    }
    let krate = match krate {
        Some(path) => quote!(#path),
        None => crate_path(),
    };
    let fields = parse_fields(&input, "FromRow")?;
    if let Some(f) = fields.iter().find(|f| {
        f.primary_key
            || f.generated
            || f.created_at
            || f.updated_at
            || f.deleted_at
            || f.version
            || f.tenant
            || f.belongs_to.is_some()
            || !f.rules.is_empty()
    }) {
        return Err(syn::Error::new(
            f.ident.span(),
            "only `column` and `skip` apply to `#[derive(FromRow)]`",
        ));
    }
    Ok(from_row_tokens(&input.ident, &fields, &krate))
}
