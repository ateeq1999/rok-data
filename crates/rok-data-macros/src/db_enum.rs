//! The `DbEnum` derive's expansion.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, LitStr, spanned::Spanned};

use crate::attrs::parse_crate_attr;
use crate::naming::rename;
pub(crate) fn expand_db_enum(input: DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.generics.span(),
            "`#[derive(DbEnum)]` does not support generics",
        ));
    }
    let mut type_name: Option<String> = None;
    let mut rule = "snake_case".to_owned();
    let krate = parse_crate_attr(&input.attrs, &["type_name", "rename_all"], |meta| {
        if meta.path.is_ident("type_name") {
            type_name = Some(meta.value()?.parse::<LitStr>()?.value());
            Ok(true)
        } else if meta.path.is_ident("rename_all") {
            rule = meta.value()?.parse::<LitStr>()?.value();
            Ok(true)
        } else {
            Ok(false)
        }
    })?;
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new(
            name.span(),
            "`#[derive(DbEnum)]` can only be used on enums",
        ));
    };
    let mut variants = Vec::new();
    for v in &data.variants {
        if !matches!(v.fields, Fields::Unit) {
            return Err(syn::Error::new(
                v.ident.span(),
                "`#[derive(DbEnum)]` variants can't have fields",
            ));
        }
        let mut stored = rename(&v.ident.to_string(), &rule)?;
        for attr in v.attrs.iter().filter(|a| a.path().is_ident("rok")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    stored = meta.value()?.parse::<LitStr>()?.value();
                    Ok(())
                } else {
                    Err(meta.error("unknown `rok` attribute on a variant; expected `rename`"))
                }
            })?;
        }
        variants.push((v.ident.clone(), stored));
    }
    let p = quote!(#krate::__private);
    let to_str = variants.iter().map(|(v, s)| quote!(#name::#v => #s));
    let from_str = variants
        .iter()
        .map(|(v, s)| quote!(#s => ::core::result::Result::Ok(#name::#v)));
    let expected = variants
        .iter()
        .map(|(_, s)| s.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let err = format!("invalid `{name}` value {{:?}} (expected one of: {expected})");
    let (type_info, compatible) = match &type_name {
        Some(t) => (
            quote!(#p::PgTypeInfo::with_name(#t)),
            quote!(*ty == <Self as #p::Type<#p::Postgres>>::type_info() || <::std::string::String as #p::Type<#p::Postgres>>::compatible(ty)),
        ),
        None => (
            quote!(<::std::string::String as #p::Type<#p::Postgres>>::type_info()),
            quote!(<::std::string::String as #p::Type<#p::Postgres>>::compatible(ty)),
        ),
    };
    Ok(quote! {
        impl #name {
            /// The value stored in the database.
            pub const fn as_str(&self) -> &'static str {
                match self { #(#to_str,)* }
            }
        }

        impl ::core::fmt::Display for #name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::core::str::FromStr for #name {
            type Err = ::std::string::String;
            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                match s {
                    #(#from_str,)*
                    other => ::core::result::Result::Err(::std::format!(#err, other)),
                }
            }
        }

        impl #p::Type<#p::Postgres> for #name {
            fn type_info() -> #p::PgTypeInfo { #type_info }
            fn compatible(ty: &#p::PgTypeInfo) -> bool { #compatible }
        }

        impl<'q> #p::Encode<'q, #p::Postgres> for #name {
            fn encode_by_ref(
                &self,
                buf: &mut #p::PgArgumentBuffer,
            ) -> ::core::result::Result<#p::IsNull, #p::BoxDynError> {
                <&str as #p::Encode<'q, #p::Postgres>>::encode_by_ref(&self.as_str(), buf)
            }
        }

        impl<'r> #p::Decode<'r, #p::Postgres> for #name {
            fn decode(value: #p::PgValueRef<'r>) -> ::core::result::Result<Self, #p::BoxDynError> {
                let s = <&str as #p::Decode<'r, #p::Postgres>>::decode(value)?;
                <Self as ::core::str::FromStr>::from_str(s).map_err(::core::convert::Into::into)
            }
        }

        #krate::impl_value!(#name);
    })
}
