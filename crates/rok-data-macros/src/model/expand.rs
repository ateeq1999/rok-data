//! The `Model` derive's expansion.

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, LitStr, Path, spanned::Spanned};

use super::from_row::from_row_tokens;
use super::{Field, Relation, Rule, parse_relations, parse_rules};
use crate::naming::{crate_path, pluralize, snake_case, unraw};
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
    let mut hooks = false;
    let mut validate_with: Option<Path> = None;
    let mut default_scope: Option<Path> = None;
    let mut relations = Vec::new();
    let mut from_row = true;
    let mut krate: Option<Path> = None;
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("rok")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("table") {
                table = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("timestamps") {
                timestamps = true;
            } else if meta.path.is_ident("soft_delete") {
                soft_delete = true;
            } else if meta.path.is_ident("hooks") {
                hooks = true;
            } else if meta.path.is_ident("validate_with") {
                validate_with = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("default_scope") {
                default_scope = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("has_many") {
                parse_relations(&meta, "HasMany", &mut relations)?;
            } else if meta.path.is_ident("has_one") {
                parse_relations(&meta, "HasOne", &mut relations)?;
            } else if meta.path.is_ident("no_from_row") {
                from_row = false;
            } else if meta.path.is_ident("crate") {
                krate = Some(meta.value()?.parse::<LitStr>()?.parse()?);
            } else {
                return Err(meta
                    .error("unknown `rok` attribute; expected `table`, `no_from_row` or `crate`"));
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
                pub fn #rel(&self) -> #krate::Select<#model> {
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
        } = r;
        let kind = Ident::new(kind, Span::call_site());
        let const_name = format_ident!("{}", unraw(rel).to_uppercase(), span = rel.span());
        let doc = format!(
            "`{name}` → [`{}`] through `{}`.",
            quote!(#model).to_string().replace(' ', ""),
            quote!(#foreign_key).to_string().replace(' ', "")
        );
        relation_items.push(quote! {
            #[doc = #doc]
            #[allow(dead_code)]
            pub const #const_name: #krate::#kind<#name, #model> = #krate::#kind::new(#foreign_key);

            #[doc = #doc]
            #[allow(dead_code)]
            pub fn #rel(&self) -> #krate::Select<#model> {
                Self::#const_name.query(self)
            }
        });
    }

    let validations: Vec<TokenStream2> = columns
        .iter()
        .flat_map(|f| {
            let ident = &f.ident;
            let field = unraw(ident);
            let v = quote!(#krate::validate);
            f.rules.iter().map(move |rule| {
                let opt = |e: &Option<syn::Expr>, cast: TokenStream2| match e {
                    Some(e) => quote!(::core::option::Option::Some((#e) as #cast)),
                    None => quote!(::core::option::Option::None),
                };
                let (code, check) = match rule {
                    Rule::Length(min, max) => {
                        let (min, max) = (opt(min, quote!(usize)), opt(max, quote!(usize)));
                        ("length", quote!(#v::length(&self.#ident, #min, #max)))
                    }
                    Rule::Range(min, max) => {
                        let (min, max) = (opt(min, quote!(f64)), opt(max, quote!(f64)));
                        ("range", quote!(#v::range(&self.#ident, #min, #max)))
                    }
                    Rule::Email => ("email", quote!(#v::email(&self.#ident))),
                    Rule::NonEmpty => ("non_empty", quote!(#v::non_empty(&self.#ident))),
                    Rule::Custom(path) => ("custom", quote!(#path(&self.#ident))),
                };
                quote! {
                    if let ::core::result::Result::Err(message) = #check {
                        errors.add(#field, #code, message);
                    }
                }
            })
        })
        .collect();
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
    let default_scope_fn = default_scope.map(|path| {
        quote! {
            fn default_scope() -> ::core::option::Option<#krate::Expr<Self>> {
                ::core::option::Option::Some(#path())
            }
        }
    });
    let hooks_impl = (!hooks).then(|| quote!(impl #krate::Hooks for #name {}));

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

        impl #name {
            #(#consts)*
            #(#relation_items)*
        }

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
                        f.belongs_to = Some((Ident::new(name, f.ident.span()), model));
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
                } else if meta.path.is_ident("column") || meta.path.is_ident("rename") {
                    f.column = meta.value()?.parse::<LitStr>()?.value();
                } else {
                    return Err(meta.error(
                        "unknown `rok` attribute; expected `primary_key`, `generated`, `column`, `skip`, `created_at`, `updated_at`, `deleted_at`, `version`, `tenant`, `validate` or `belongs_to`",
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
