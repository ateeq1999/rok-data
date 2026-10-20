//! The `DbNewtype` derive's expansion.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, spanned::Spanned};

use crate::attrs::parse_crate_attr;
pub(crate) fn expand_db_newtype(input: DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.generics.span(),
            "`#[derive(DbNewtype)]` does not support generics",
        ));
    }
    let krate = parse_crate_attr(&input.attrs, &[], |_| Ok(false))?;
    let inner = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Unnamed(f) if f.unnamed.len() == 1 => f.unnamed[0].ty.clone(),
            _ => None.ok_or_else(|| {
                syn::Error::new(
                    name.span(),
                    "`#[derive(DbNewtype)]` requires a struct with exactly one unnamed field",
                )
            })?,
        },
        _ => {
            return Err(syn::Error::new(
                name.span(),
                "`#[derive(DbNewtype)]` can only be used on structs",
            ));
        }
    };
    let p = quote!(#krate::__private);
    Ok(quote! {
        impl #p::Type<#p::Postgres> for #name {
            fn type_info() -> #p::PgTypeInfo { <#inner as #p::Type<#p::Postgres>>::type_info() }
            fn compatible(ty: &#p::PgTypeInfo) -> bool { <#inner as #p::Type<#p::Postgres>>::compatible(ty) }
        }

        impl<'q> #p::Encode<'q, #p::Postgres> for #name {
            fn encode_by_ref(
                &self,
                buf: &mut #p::PgArgumentBuffer,
            ) -> ::core::result::Result<#p::IsNull, #p::BoxDynError> {
                <#inner as #p::Encode<'q, #p::Postgres>>::encode_by_ref(&self.0, buf)
            }
        }

        impl<'r> #p::Decode<'r, #p::Postgres> for #name {
            fn decode(value: #p::PgValueRef<'r>) -> ::core::result::Result<Self, #p::BoxDynError> {
                <#inner as #p::Decode<'r, #p::Postgres>>::decode(value).map(#name)
            }
        }

        #krate::impl_value!(#name);
    })
}
