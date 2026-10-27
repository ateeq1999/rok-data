//! `#[derive(Input)]`: an input struct's transforms and rules, checked by
//! `validate` before it becomes rows (workspace ADR 0021).

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, Path};

use super::Field;
use super::expand::parse_fields;
use super::rules::{custom_database_steps, database_rule_builders, field_checks, transform_steps};
use crate::naming::crate_path;

pub(crate) fn expand(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match syn::parse::<DeriveInput>(input).and_then(|input| tokens(&input)) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn tokens(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let mut validate_with: Option<Path> = None;
    let mut validate_with_database: Option<Path> = None;
    let mut krate = crate_path();
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("rok")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("validate_with") {
                validate_with = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("validate_with_database") {
                validate_with_database = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("crate") {
                let path: Path = meta.value()?.parse()?;
                krate = quote!(#path);
            } else {
                return Err(meta.error(
                    "unknown `rok` attribute on an input; expected `validate_with`, `validate_with_database` or `crate`",
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
    let builders = database_rule_builders(&fields, &krate);
    let custom = custom_database_steps(&fields);
    let record_database = validate_with_database.map(|path| {
        quote! {
            if let ::core::result::Result::Err(more) = #path(self, journey.reborrow()).await {
                found.merge(more);
            }
        }
    });
    let database_rules_fn = (!builders.is_empty()).then(|| {
        quote! {
            fn __database_rules(&self) -> ::std::vec::Vec<#krate::validate::DatabaseRule> {
                [#(#builders),*].into_iter().flatten().collect()
            }
        }
    });
    let custom_fn = (!custom.is_empty() || record_database.is_some()).then(|| {
        quote! {
            async fn __custom_database(
                &self,
                journey: &mut #krate::Journey<'_>,
                failed: &#krate::ValidationErrors,
            ) -> #krate::Result<#krate::ValidationErrors> {
                let _ = failed;
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

            #database_rules_fn
            #custom_fn
        }
    })
}
