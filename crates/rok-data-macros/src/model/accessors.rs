//! Accessors the derive writes beside the columns: a value object read
//! from and written to several columns (`#[rok(value_object(…))]`), and a
//! setter that transforms what it writes (`#[rok(set_with = …)]`).

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::{Ident, Path, Token, Type, parenthesized};

use super::Field;
use crate::naming::unraw;

/// `value_object(address: Address = (street, city, zip))`: `address()`
/// builds an `Address` from the three columns, `set_address(…)` writes
/// them back.
pub(super) struct ValueObject {
    name: Ident,
    ty: Type,
    columns: Vec<Ident>,
}

/// Parses one `value_object(name: Type = (field, …))` attribute.
pub(super) fn parse_value_object(
    meta: &syn::meta::ParseNestedMeta<'_>,
    out: &mut Vec<ValueObject>,
) -> syn::Result<()> {
    meta.parse_nested_meta(|inner| {
        let name = inner
            .path
            .get_ident()
            .cloned()
            .ok_or_else(|| inner.error("expected `name: Type = (field, …)`"))?;
        inner.input.parse::<Token![:]>()?;
        let ty: Type = inner.input.parse()?;
        inner.input.parse::<Token![=]>()?;
        let content;
        parenthesized!(content in inner.input);
        let columns =
            Punctuated::<Ident, Token![,]>::parse_terminated_with(&content, Ident::parse)?;
        if columns.is_empty() {
            return Err(inner.error("a value object needs at least one field"));
        }
        out.push(ValueObject {
            name,
            ty,
            columns: columns.into_iter().collect(),
        });
        Ok(())
    })
}

/// The accessors of `objects` and of every field with a `set_with`.
pub(super) fn tokens(objects: &[ValueObject], fields: &[Field]) -> syn::Result<TokenStream2> {
    let mut items = Vec::new();
    for object in objects {
        let ValueObject { name, ty, columns } = object;
        let types = columns
            .iter()
            .map(|column| {
                fields
                    .iter()
                    .find(|f| f.ident == *column)
                    .map(|f| &f.ty)
                    .ok_or_else(|| {
                        syn::Error::new(column.span(), format!("no field `{column}` on the model"))
                    })
            })
            .collect::<syn::Result<Vec<_>>>()?;
        let setter = format_ident!("set_{}", unraw(name), span = name.span());
        let getter_doc = format!(
            "The `{}` value object, built from {}.",
            unraw(name),
            columns
                .iter()
                .map(|c| format!("`{c}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let setter_doc = format!("Writes a `{}` back into its fields.", unraw(name));
        items.push(quote! {
            #[doc = #getter_doc]
            #[allow(dead_code)]
            pub fn #name(&self) -> #ty {
                <#ty as ::core::convert::From<(#(#types,)*)>>::from((#(::core::clone::Clone::clone(&self.#columns),)*))
            }

            #[doc = #setter_doc]
            #[allow(dead_code)]
            pub fn #setter(&mut self, value: #ty) {
                let (#(#columns,)*): (#(#types,)*) = ::core::convert::Into::into(value);
                #(self.#columns = #columns;)*
            }
        });
    }
    for field in fields {
        let Some(transform) = &field.set_with else {
            continue;
        };
        let ident = &field.ident;
        let ty = &field.ty;
        let setter = format_ident!("set_{}", unraw(ident), span = ident.span());
        let doc = format!(
            "Sets `{}` to `value` passed through `{}`.",
            unraw(ident),
            quote!(#transform).to_string().replace(' ', "")
        );
        items.push(quote! {
            #[doc = #doc]
            #[allow(dead_code)]
            pub fn #setter(&mut self, value: #ty) {
                self.#ident = #transform(value);
            }
        });
    }
    Ok(quote!(#(#items)*))
}

/// A `set_with = path` field attribute's path.
pub(super) fn parse_set_with(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<Path> {
    meta.value()?.parse()
}
