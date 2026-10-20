//! The `#[rok(crate = "…")]` attribute every derive understands.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{LitStr, Path};

use crate::naming::crate_path;

pub(crate) fn parse_crate_attr(
    attrs: &[syn::Attribute],
    allowed: &[&str],
    mut other: impl FnMut(&syn::meta::ParseNestedMeta<'_>) -> syn::Result<bool>,
) -> syn::Result<TokenStream2> {
    let mut krate: Option<Path> = None;
    for attr in attrs.iter().filter(|a| a.path().is_ident("rok")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                krate = Some(meta.value()?.parse::<LitStr>()?.parse()?);
                Ok(())
            } else if other(&meta)? {
                Ok(())
            } else {
                Err(meta.error(format!(
                    "unknown `rok` attribute; expected one of: crate, {}",
                    allowed.join(", ")
                )))
            }
        })?;
    }
    Ok(match krate {
        Some(path) => quote!(#path),
        None => crate_path(),
    })
}
