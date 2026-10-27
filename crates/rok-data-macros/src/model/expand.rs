//! The `Model` derive's expansion.

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, LitStr, Path, spanned::Spanned};

use super::conventions::{self, Conventions};
use super::from_row::from_row_tokens;
use super::scopes;
use super::{Field, Relation, parse_many_to_many, parse_relations, parse_rules, parse_through};
use crate::naming::{crate_path, method_ident, pluralize, relation_ident, snake_case, unraw};
pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.generics.span(),
            "`#[derive(Model)]` does not support generic structs",
        ));
    }

    let mut table = None;
    let mut timestamps = false;
    let mut soft_delete = false;
    let mut no_timestamps = false;
    let mut no_soft_delete = false;
    let mut hooks = false;
    let mut validate_with: Option<Path> = None;
    let mut default_scopes: Vec<Path> = Vec::new();
    let mut per_page: Option<syn::LitInt> = None;
    let mut relations = Vec::new();
    let mut named_scopes = Vec::new();
    let mut value_objects = Vec::new();
    let mut from_row = true;
    let mut krate: Option<Path> = None;
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("rok")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("table") {
                table = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("timestamps") {
                timestamps = true;
            } else if meta.path.is_ident("no_timestamps") {
                no_timestamps = true;
            } else if meta.path.is_ident("soft_delete") {
                soft_delete = true;
            } else if meta.path.is_ident("no_soft_delete") {
                no_soft_delete = true;
            } else if meta.path.is_ident("hooks") {
                hooks = true;
            } else if meta.path.is_ident("validate_with") {
                validate_with = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("default_scope") {
                default_scopes.push(meta.value()?.parse()?);
            } else if meta.path.is_ident("per_page") {
                let value: syn::LitInt = meta.value()?.parse()?;
                if value.base10_parse::<u64>()? == 0 {
                    return Err(syn::Error::new(value.span(), "`per_page` must be greater than 0"));
                }
                per_page = Some(value);
            } else if meta.path.is_ident("scopes") {
                scopes::parse_scopes(&meta, &mut named_scopes)?;
            } else if meta.path.is_ident("value_object") {
                super::accessors::parse_value_object(&meta, &mut value_objects)?;
            } else if meta.path.is_ident("has_many") {
                parse_relations(&meta, "HasMany", &mut relations)?;
            } else if meta.path.is_ident("has_one") {
                parse_relations(&meta, "HasOne", &mut relations)?;
            } else if meta.path.is_ident("has_many_through") {
                parse_through(&meta, "HasManyThrough", &mut relations)?;
            } else if meta.path.is_ident("has_one_through") {
                parse_through(&meta, "HasOneThrough", &mut relations)?;
            } else if meta.path.is_ident("belongs_to_many") {
                parse_many_to_many(&meta, &mut relations)?;
            } else if meta.path.is_ident("no_from_row") {
                from_row = false;
            } else if meta.path.is_ident("crate") {
                krate = Some(meta.value()?.parse::<LitStr>()?.parse()?);
            } else {
                return Err(meta.error(
                    "unknown `rok` attribute; expected `table`, `timestamps`, `no_timestamps`, `soft_delete`, `no_soft_delete`, `hooks`, `validate_with`, `default_scope`, `per_page`, `scopes`, `has_many`, `has_one`, `has_many_through`, `has_one_through`, `belongs_to_many`, `value_object`, `no_from_row` or `crate`",
                ));
            }
            Ok(())
        })?;
    }
    let table = table.unwrap_or_else(|| pluralize(&snake_case(&name.to_string())));
    let krate = match krate {
        Some(path) => quote!(#path),
        None => crate_path(),
    };

    let fields = parse_fields(&input, "Model")?;

    let columns: Vec<&Field> = fields.iter().filter(|f| !f.skip).collect();
    // The conventions the model carries: the field names are the signal, and
    // the opt-outs turn the inference off (workspace ADR 0015).
    let conventions = conventions::decide(
        name,
        &columns,
        timestamps,
        soft_delete,
        no_timestamps,
        no_soft_delete,
    )?;
    let Conventions {
        timestamps,
        soft_delete,
    } = conventions;
    let mut pks: Vec<&Field> = columns.iter().filter(|f| f.primary_key).copied().collect();
    if pks.is_empty() {
        // Tables without a primary key are legal: the key constants and
        // `primary_key` then carry an empty key, and generated code for
        // such tables never looks records up by id.
        if let Some(id) = columns.iter().find(|f| f.ident == "id") {
            pks.push(id);
        }
    }
    let pk_columns = pks.iter().map(|f| &f.column);

    let find_ts = |flag: fn(&Field) -> bool,
                   default: &str,
                   implied: bool,
                   container: &str|
     -> syn::Result<Option<String>> {
        let mut marked = columns.iter().filter(|f| flag(f));
        match (marked.next(), marked.next()) {
            (Some(_), Some(second)) => Err(syn::Error::new(
                second.ident.span(),
                format!("only one field can be `#[rok({default})]`"),
            )),
            (Some(f), None) => Ok(Some(f.column.clone())),
            (None, _) if implied => columns
                .iter()
                .find(|f| f.ident == default)
                .map(|f| Some(f.column.clone()))
                .ok_or_else(|| {
                    syn::Error::new(
                        name.span(),
                        format!("`#[rok({container})]` requires a `{default}` field (or mark one with `#[rok({default})]`)"),
                    )
                }),
            (None, _) => Ok(None),
        }
    };
    let created_at = find_ts(|f| f.created_at, "created_at", timestamps, "timestamps")?;
    let updated_at = find_ts(|f| f.updated_at, "updated_at", timestamps, "timestamps")?;
    let deleted_at = find_ts(|f| f.deleted_at, "deleted_at", soft_delete, "soft_delete")?;
    let version = find_ts(|f| f.version, "version", false, "")?;
    let tenant = find_ts(|f| f.tenant, "tenant", false, "")?;
    // A managed column is written by rok-data: the database must not fill it
    // as well. The columns the conventions infer are managed like the marked
    // ones, so a `generated` column cannot be one of them.
    for column in [
        created_at.as_deref(),
        updated_at.as_deref(),
        deleted_at.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(field) = columns.iter().find(|f| f.column == column)
            && field.generated
        {
            return Err(syn::Error::new(
                field.ident.span(),
                "managed columns (timestamps, `deleted_at`, `version`) are written by rok-data and can't be `generated`",
            ));
        }
    }
    let opt = |v: Option<String>| match v {
        Some(c) => quote!(::core::option::Option::Some(#c)),
        None => quote!(::core::option::Option::None),
    };
    let created_at = opt(created_at);
    let updated_at = opt(updated_at);
    let deleted_at = opt(deleted_at);
    let version = opt(version);
    let tenant = opt(tenant);

    let value_arms = columns.iter().map(|f| {
        let ident = &f.ident;
        let column = &f.column;
        quote!(#column => ::core::option::Option::Some(#krate::Value::from(&self.#ident)))
    });

    let mut relation_items = Vec::new();
    for f in &columns {
        if let Some((rel, model)) = &f.belongs_to {
            let accessor = method_ident(rel)?;
            let const_name = format_ident!("{}", unraw(rel).to_uppercase(), span = rel.span());
            let fk_const =
                format_ident!("{}", unraw(&f.ident).to_uppercase(), span = f.ident.span());
            let doc = format!(
                "`{name}` belongs to [`{}`] through `{}`.",
                quote!(#model).to_string().replace(' ', ""),
                f.column
            );
            relation_items.push(quote! {
                #[doc = #doc]
                #[allow(dead_code)]
                pub const #const_name: #krate::BelongsTo<#name, #model> = #krate::BelongsTo::new(Self::#fk_const);

                #[doc = #doc]
                #[allow(dead_code)]
                pub fn #accessor(&self) -> #krate::Select<#model> {
                    Self::#const_name.query(self)
                }
            });
        }
    }
    for r in &relations {
        let Relation {
            kind,
            name: rel,
            model,
            foreign_key,
            through,
            pivot,
        } = r;
        let accessor = method_ident(rel)?;
        let kind = Ident::new(kind, Span::call_site());
        let const_name = format_ident!("{}", unraw(rel).to_uppercase(), span = rel.span());
        let doc = format!(
            "`{name}` → [`{}`] through `{}`.",
            quote!(#model).to_string().replace(' ', ""),
            quote!(#foreign_key).to_string().replace(' ', "")
        );
        let constant = match (through, pivot) {
            (_, Some(pivot)) => {
                let target = model
                    .segments
                    .last()
                    .map(|s| s.ident.to_string())
                    .unwrap_or_default();
                let key = |given: &Option<LitStr>, model: &str| {
                    given
                        .as_ref()
                        .map_or_else(|| format!("{}_id", snake_case(model)), LitStr::value)
                };
                let table = &pivot.table;
                let foreign = key(&pivot.foreign_key, &unraw(name));
                let related = key(&pivot.related_key, &target);
                quote! {
                    pub const #const_name: #krate::#kind<#name, #model> =
                        #krate::#kind::new(#table, #foreign, #related);
                }
            }
            (None, None) => quote! {
                pub const #const_name: #krate::#kind<#name, #model> = #krate::#kind::new(#foreign_key);
            },
            (Some((through_model, through_key)), None) => quote! {
                pub const #const_name: #krate::#kind<#name, #through_model, #model> =
                    #krate::#kind::new(#through_key, #foreign_key);
            },
        };
        relation_items.push(quote! {
            #[doc = #doc]
            #[allow(dead_code)]
            #constant

            #[doc = #doc]
            #[allow(dead_code)]
            pub fn #accessor(&self) -> #krate::Select<#model> {
                Self::#const_name.query(self)
            }
        });
    }

    let validations = super::rules::field_checks(&columns, &krate);
    let validate_fn = (!validations.is_empty() || validate_with.is_some()).then(|| {
        let record = validate_with.as_ref().map(|path| {
            quote! {
                if let ::core::result::Result::Err(more) = #path(self) {
                    errors.merge(more);
                }
            }
        });
        quote! {
            fn validate(&self) -> ::core::result::Result<(), #krate::ValidationErrors> {
                let mut errors = #krate::ValidationErrors::new();
                #(#validations)*
                #record
                errors.into_result()
            }
        }
    });
    // Repeated `default_scope`s all apply, combined with `AND`.
    let default_scope_fn = match default_scopes.as_slice() {
        [] => None,
        [path] => Some(quote! {
            fn default_scope() -> ::core::option::Option<#krate::Expr<Self>> {
                ::core::option::Option::Some(#path())
            }
        }),
        paths => Some(quote! {
            fn default_scope() -> ::core::option::Option<#krate::Expr<Self>> {
                ::core::option::Option::Some(#krate::Expr::all_of([#(#paths()),*]))
            }
        }),
    };
    let per_page_const = per_page.map(|n| quote! { const PER_PAGE: u64 = #n; });
    let hooks_impl = (!hooks).then(|| quote!(impl #krate::Hooks for #name {}));
    let accessor_items = super::accessors::tokens(&value_objects, &fields)?;
    let scope_entries = scopes::entries(name, &named_scopes, &krate);
    let scope_trait = scopes::trait_declaration(name, &named_scopes, &krate);
    let scope_checks = scopes::signature_checks(name, &named_scopes, &krate);

    let pk_column = match pks.first() {
        Some(f) => {
            let column = &f.column;
            quote!(#column)
        }
        None => quote!(""),
    };
    let pk_ident = pks.first().map(|f| &f.ident);
    let primary_key_fn = match pk_ident {
        Some(ident) => quote! {
            fn primary_key(&self) -> #krate::Value {
                #krate::Value::from(&self.#ident)
            }
        },
        None => quote! {
            fn primary_key(&self) -> #krate::Value {
                #krate::Value::I64(None)
            }
        },
    };
    let column_names = columns.iter().map(|f| &f.column);
    let generated = columns.iter().filter(|f| f.generated).map(|f| &f.column);
    let values = columns.iter().map(|f| {
        let ident = &f.ident;
        let column = &f.column;
        quote!((#column, #krate::Value::from(&self.#ident)))
    });
    let consts = columns.iter().map(|f| {
        let const_name = format_ident!("{}", unraw(&f.ident).to_uppercase(), span = f.ident.span());
        let column = &f.column;
        let doc = format!("The `{column}` column.");
        quote! {
            #[doc = #doc]
            #[allow(dead_code)]
            pub const #const_name: #krate::Column<#name> = #krate::Column::new(#column);
        }
    });

    let from_row_impl = from_row.then(|| from_row_tokens(name, &fields, &krate));
    let from_row_at_fn = from_row.then(|| {
        let mut index = 0usize;
        let inits = fields.iter().map(|f| {
            let ident = &f.ident;
            if f.skip {
                quote!(#ident: ::core::default::Default::default())
            } else {
                let i = index;
                index += 1;
                quote!(#ident: #krate::__private::Row::try_get(row, offset + #i)?)
            }
        });
        let inits: Vec<_> = inits.collect();
        quote! {
            fn from_row_at(
                row: &#krate::__private::PgRow,
                offset: usize,
            ) -> ::core::result::Result<Self, #krate::__private::SqlxError> {
                ::core::result::Result::Ok(Self { #(#inits,)* })
            }
        }
    });

    Ok(quote! {
        #from_row_impl
        #hooks_impl
        #(#scope_checks)*

        impl #name {
            #(#consts)*
            #(#relation_items)*
            #(#scope_entries)*
            #accessor_items
        }

        #scope_trait

        impl #krate::Model for #name {
            const TABLE: &'static str = #table;
            const PRIMARY_KEY: &'static str = #pk_column;
            const PRIMARY_KEYS: &'static [&'static str] = &[#(#pk_columns),*];
            const COLUMNS: &'static [&'static str] = &[#(#column_names),*];
            const GENERATED: &'static [&'static str] = &[#(#generated),*];
            const CREATED_AT_COLUMN: ::core::option::Option<&'static str> = #created_at;
            const UPDATED_AT_COLUMN: ::core::option::Option<&'static str> = #updated_at;
            const DELETED_AT_COLUMN: ::core::option::Option<&'static str> = #deleted_at;
            const VERSION_COLUMN: ::core::option::Option<&'static str> = #version;
            const TENANT_COLUMN: ::core::option::Option<&'static str> = #tenant;
            #per_page_const

            #primary_key_fn

            fn values(&self) -> ::std::vec::Vec<(&'static str, #krate::Value)> {
                ::std::vec![#(#values),*]
            }

            #validate_fn
            #default_scope_fn
            #from_row_at_fn

            fn value_of(&self, column: &str) -> ::core::option::Option<#krate::Value> {
                match column {
                    #(#value_arms,)*
                    _ => ::core::option::Option::None,
                }
            }
        }

        impl #krate::ModelWrite for #name {}
    })
}

pub(crate) fn parse_fields(input: &DeriveInput, derive: &str) -> syn::Result<Vec<Field>> {
    let named = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new(
                    input.ident.span(),
                    format!("`#[derive({derive})]` requires a struct with named fields"),
                ));
            }
        },
        _ => {
            return Err(syn::Error::new(
                input.ident.span(),
                format!("`#[derive({derive})]` can only be used on structs"),
            ));
        }
    };

    let mut fields = Vec::new();
    for field in named {
        let ident = field.ident.clone().expect("named field");
        let mut f = Field {
            column: unraw(&ident),
            ident,
            primary_key: false,
            generated: false,
            skip: false,
            created_at: false,
            updated_at: false,
            deleted_at: false,
            version: false,
            tenant: false,
            belongs_to: None,
            rules: Vec::new(),
            ty: field.ty.clone(),
            set_with: None,
            transforms: Vec::new(),
        };
        for attr in field.attrs.iter().filter(|a| a.path().is_ident("rok")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("primary_key") {
                    f.primary_key = true;
                } else if meta.path.is_ident("generated") || meta.path.is_ident("auto") {
                    f.generated = true;
                } else if meta.path.is_ident("skip") {
                    f.skip = true;
                } else if meta.path.is_ident("created_at") {
                    f.created_at = true;
                } else if meta.path.is_ident("updated_at") {
                    f.updated_at = true;
                } else if meta.path.is_ident("deleted_at") {
                    f.deleted_at = true;
                } else if meta.path.is_ident("version") {
                    f.version = true;
                } else if meta.path.is_ident("tenant") {
                    f.tenant = true;
                } else if meta.path.is_ident("validate") {
                    parse_rules(&meta, &mut f.rules)?;
                } else if meta.path.is_ident("belongs_to") {
                    if meta.input.peek(syn::Token![=]) {
                        let model: Path = meta.value()?.parse()?;
                        let field = unraw(&f.ident);
                        let name = field.strip_suffix("_id").ok_or_else(|| {
                            meta.error(format!(
                                "can't derive a relation name from `{field}`; use `belongs_to(name = Model)`"
                            ))
                        })?;
                        f.belongs_to = Some((relation_ident(name, f.ident.span())?, model));
                    } else {
                        meta.parse_nested_meta(|inner| {
                            let name = inner.path.get_ident().cloned().ok_or_else(|| {
                                inner.error("expected `belongs_to(name = Model)`")
                            })?;
                            let model: Path = inner.value()?.parse()?;
                            f.belongs_to = Some((name, model));
                            Ok(())
                        })?;
                    }
                } else if meta.path.is_ident("transform") {
                    if derive == "Model" {
                        return Err(meta.error(
                            "`transform` belongs to input: use `#[derive(Input)]` for data on its way to becoming rows",
                        ));
                    }
                    super::rules::parse_transforms(&meta, &mut f.transforms)?;
                } else if meta.path.is_ident("set_with") {
                    f.set_with = Some(super::accessors::parse_set_with(&meta)?);
                } else if meta.path.is_ident("column") || meta.path.is_ident("rename") {
                    f.column = meta.value()?.parse::<LitStr>()?.value();
                } else {
                    return Err(meta.error(
                        "unknown `rok` attribute; expected `primary_key`, `generated`, `column`, `skip`, `created_at`, `updated_at`, `deleted_at`, `version`, `tenant`, `validate`, `transform`, `set_with` or `belongs_to`",
                    ));
                }
                Ok(())
            })?;
        }
        let managed = f.created_at || f.updated_at || f.deleted_at || f.version || f.tenant;
        if f.skip
            && (f.primary_key
                || f.generated
                || managed
                || f.belongs_to.is_some()
                || !f.rules.is_empty())
        {
            return Err(syn::Error::new(
                f.ident.span(),
                "`skip` fields are not columns and can't have other `rok` attributes",
            ));
        }
        if derive == "Model"
            && f.rules
                .iter()
                .any(|rule| matches!(rule, super::Rule::Database(_)))
        {
            return Err(syn::Error::new(
                f.ident.span(),
                "database rules (`unique`, `exists`, `database`) belong to input: use `#[derive(Input)]`; the table's constraint already guards this model's writes",
            ));
        }
        if managed && f.generated {
            return Err(syn::Error::new(
                f.ident.span(),
                "managed columns (timestamps, `deleted_at`, `version`) are written by rok-data and can't be `generated`",
            ));
        }
        fields.push(f);
    }
    Ok(fields)
}
