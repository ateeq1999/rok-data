//! `#[derive(Input)]`: an input struct's transforms and rules, checked by
//! `validate` before it becomes rows (workspace ADR 0021).

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, Path};

use super::Field;
use super::expand::parse_fields;
use super::rules::{
    custom_database_steps, database_rule_steps, field_checks, schema_steps, transform_steps,
};
use crate::naming::crate_path;

pub(crate) fn expand(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match syn::parse::<DeriveInput>(input).and_then(|input| tokens(&input)) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn tokens(input: &DeriveInput) -> syn::Result<TokenStream2> {
    if let syn::Data::Enum(data) = &input.data {
        return enum_tokens(input, data);
    }
    let mut validate_with: Option<Path> = None;
    let mut validate_with_database: Option<Path> = None;
    let mut context: Option<syn::Type> = None;
    let mut krate = crate_path();
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("rok")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("validate_with") {
                validate_with = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("validate_with_database") {
                validate_with_database = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("context") {
                context = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("crate") {
                let path: Path = meta.value()?.parse()?;
                krate = quote!(#path);
            } else {
                return Err(meta.error(
                    "unknown `rok` attribute on an input; expected `validate_with`, `validate_with_database`, `context` or `crate`",
                ));
            }
            Ok(())
        })?;
    }
    let fields = parse_fields(input, "Input")?;
    if let Some(column_only) = fields.iter().find(|f| f.is_column_only()) {
        return Err(syn::Error::new(
            column_only.ident.span(),
            "an input field takes `validate(…)` and `transform(…)`; keys, timestamps and relations belong to a `#[derive(Model)]`",
        ));
    }
    let fields: Vec<&Field> = fields.iter().filter(|f| !f.skip).collect();
    let checks = field_checks(&fields, &krate);
    let steps = transform_steps(&fields, &krate);
    let with_context = context.is_some();
    let context_type = context.map_or_else(|| quote!(()), |ty| quote!(#ty));
    let record_context = with_context.then(|| quote!(, context));
    let rule_steps = database_rule_steps(&fields, &krate, with_context);
    let schema = schema_steps(&fields, &krate);
    let custom = custom_database_steps(&fields, with_context);
    let record_database = validate_with_database.map(|path| {
        quote! {
            if let ::core::result::Result::Err(more) = #path(self, journey.reborrow() #record_context).await {
                found.merge(more);
            }
        }
    });
    let database_rules_fn = (!rule_steps.is_empty()).then(|| {
        quote! {
            fn __database_rules(&self, context: &Self::Context) -> ::std::vec::Vec<#krate::validate::DatabaseRule> {
                let _ = context;
                let mut rules = ::std::vec::Vec::new();
                #(#rule_steps)*
                rules
            }
        }
    });
    let custom_fn = (!custom.is_empty() || record_database.is_some()).then(|| {
        quote! {
            async fn __custom_database(
                &self,
                journey: &mut #krate::Journey<'_>,
                failed: &#krate::ValidationErrors,
                context: &Self::Context,
            ) -> #krate::Result<#krate::ValidationErrors> {
                let _ = (failed, context);
                let mut found = #krate::ValidationErrors::new();
                #(#custom)*
                #record_database
                ::core::result::Result::Ok(found)
            }
        }
    });
    let record = validate_with.map(|path| {
        quote! {
            if let ::core::result::Result::Err(more) = #path(self) {
                errors.merge(more);
            }
        }
    });
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mutable = (!steps.is_empty()).then(|| quote!(mut));
    Ok(quote! {
        impl #impl_generics #krate::Input for #name #ty_generics #where_clause {
            type Context = #context_type;

            fn __transform(#mutable self) -> Self {
                #(#steps)*
                self
            }

            fn __check(&self) -> #krate::ValidationErrors {
                let mut errors = #krate::ValidationErrors::new();
                #(#checks)*
                #record
                errors
            }

            fn __schema() -> #krate::validate::Schema {
                #[allow(unused_imports)]
                use #krate::validate::schema::{ProbeAny as _, ProbeTyped as _};
                #[allow(unused_mut)]
                let mut properties = ::std::vec::Vec::new();
                #[allow(unused_mut)]
                let mut required = ::std::vec::Vec::new();
                #(#schema)*
                #krate::validate::Schema::object(properties, required)
            }

            #database_rules_fn
            #custom_fn
        }
    })
}

/// An enum input — one variant per shape, each wrapping an input of its
/// own (`enum Body { Person(PersonInput), Company(CompanyInput) }`): every
/// step delegates to the variant's input.
fn enum_tokens(input: &DeriveInput, data: &syn::DataEnum) -> syn::Result<TokenStream2> {
    let mut krate = crate_path();
    let mut context = quote!(());
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("rok")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                let path: Path = meta.value()?.parse()?;
                krate = quote!(#path);
                Ok(())
            } else if meta.path.is_ident("context") {
                let ty: syn::Type = meta.value()?.parse()?;
                context = quote!(#ty);
                Ok(())
            } else {
                Err(meta.error(
                    "an enum input takes only `crate` and `context`; its variants' inputs carry the rules",
                ))
            }
        })?;
    }
    let mut variants = Vec::new();
    let mut inner = Vec::new();
    for variant in &data.variants {
        match &variant.fields {
            syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                variants.push(&variant.ident);
                inner.push(&fields.unnamed[0].ty);
            }
            _ => {
                return Err(syn::Error::new(
                    variant.ident.span(),
                    "an enum input's variants each wrap one input: `Variant(VariantInput)`",
                ));
            }
        }
    }
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #krate::Input for #name #ty_generics #where_clause {
            type Context = #context;

            fn __transform(self) -> Self {
                match self {
                    #(Self::#variants(inner) => Self::#variants(#krate::Input::__transform(inner)),)*
                }
            }

            fn __check(&self) -> #krate::ValidationErrors {
                match self {
                    #(Self::#variants(inner) => #krate::Input::__check(inner),)*
                }
            }

            fn __schema() -> #krate::validate::Schema {
                #krate::validate::Schema::one_of(::std::vec![#(<#inner as #krate::Input>::__schema()),*])
            }

            fn __database_rules(&self, context: &Self::Context) -> ::std::vec::Vec<#krate::validate::DatabaseRule> {
                match self {
                    #(Self::#variants(inner) => #krate::Input::__database_rules(inner, context),)*
                }
            }

            async fn __custom_database(
                &self,
                journey: &mut #krate::Journey<'_>,
                failed: &#krate::ValidationErrors,
                context: &Self::Context,
            ) -> #krate::Result<#krate::ValidationErrors> {
                match self {
                    #(Self::#variants(inner) => #krate::Input::__custom_database(inner, journey, failed, context).await,)*
                }
            }
        }
    })
}
