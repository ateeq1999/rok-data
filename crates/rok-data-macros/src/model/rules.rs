//! The field rules and transforms both derives check: `#[rok(validate(…))]`
//! and `#[rok(transform(…))]`, turned into the statements of a
//! `validate` / `__check` body and a `__transform` body.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Ident, Path};

use super::{Field, Rule};
use crate::naming::unraw;

/// One `#[rok(transform(…))]` step.
pub(crate) enum Transform {
    /// `trim`, `lowercase`, `uppercase`, `normalize_email`.
    Builtin(Ident),
    /// `custom = path`, a `fn(T) -> T`.
    Custom(Path),
}

/// The built-in transforms' names.
const BUILTIN_TRANSFORMS: &[&str] = &["trim", "lowercase", "uppercase", "normalize_email"];

/// Parses `transform(trim, lowercase, custom = path)` into `out`, in order.
pub(crate) fn parse_transforms(
    meta: &syn::meta::ParseNestedMeta<'_>,
    out: &mut Vec<Transform>,
) -> syn::Result<()> {
    meta.parse_nested_meta(|inner| {
        if inner.path.is_ident("custom") {
            out.push(Transform::Custom(inner.value()?.parse()?));
            return Ok(());
        }
        match inner.path.get_ident() {
            Some(name) if BUILTIN_TRANSFORMS.contains(&name.to_string().as_str()) => {
                out.push(Transform::Builtin(name.clone()));
                Ok(())
            }
            _ => Err(inner.error(
                "unknown transform; expected `trim`, `lowercase`, `uppercase`, `normalize_email` or `custom = path`",
            )),
        }
    })
}

/// The statements that add every failed field rule of `fields` to a
/// `ValidationErrors` named `errors`, reading the values from `self`.
pub(crate) fn field_checks(fields: &[&Field], krate: &TokenStream2) -> Vec<TokenStream2> {
    fields
        .iter()
        .flat_map(|f| {
            let ident = &f.ident;
            let field = unraw(ident);
            let v = quote!(#krate::validate);
            f.rules.iter().filter_map(move |rule| {
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
                    // Database rules run against a session, after these.
                    Rule::Database(_) => return None,
                };
                Some(quote! {
                    if let ::core::result::Result::Err(message) = #check {
                        errors.add(#field, #code, message);
                    }
                })
            })
        })
        .collect()
}

/// The statements that run every field's transforms on `self`, in order.
pub(crate) fn transform_steps(fields: &[&Field], krate: &TokenStream2) -> Vec<TokenStream2> {
    fields
        .iter()
        .flat_map(|f| {
            let ident = &f.ident;
            f.transforms.iter().map(move |transform| match transform {
                Transform::Builtin(name) => {
                    quote!(self.#ident = #krate::validate::transform::#name(self.#ident);)
                }
                Transform::Custom(path) => quote!(self.#ident = #path(self.#ident);),
            })
        })
        .collect()
}

/// A database rule on an input field (workspace ADR 0021).
pub(crate) enum DatabaseRule {
    /// `unique(T::COLUMN, except = field, case_insensitive, scope = path)`.
    Unique {
        column: Path,
        except: Option<Ident>,
        case_insensitive: bool,
        scope: Option<Path>,
    },
    /// `exists(T::COLUMN, scope = path)`.
    Exists { column: Path, scope: Option<Path> },
    /// `database = path`: `async fn(&T, Journey<'_>) -> Result<(), String>`.
    Custom(Path),
}

/// `unique(…)`, `exists(…)` or `database = path`; `None` for any other rule.
pub(crate) fn parse_database_rule(
    meta: &syn::meta::ParseNestedMeta<'_>,
) -> syn::Result<Option<DatabaseRule>> {
    if meta.path.is_ident("database") {
        return Ok(Some(DatabaseRule::Custom(meta.value()?.parse()?)));
    }
    let unique = meta.path.is_ident("unique");
    if !unique && !meta.path.is_ident("exists") {
        return Ok(None);
    }
    let (mut column, mut except, mut case_insensitive, mut scope) = (None, None, false, None);
    meta.parse_nested_meta(|option| {
        if option.path.is_ident("scope") {
            scope = Some(option.value()?.parse()?);
        } else if unique && option.path.is_ident("except") {
            except = Some(option.value()?.parse()?);
        } else if unique && option.path.is_ident("case_insensitive") {
            case_insensitive = true;
        } else if option.path.segments.len() >= 2 && column.is_none() {
            column = Some(option.path.clone());
        } else {
            return Err(option.error(if unique {
                "expected `Model::COLUMN`, then `except = field`, `case_insensitive` or `scope = path`"
            } else {
                "expected `Model::COLUMN`, then `scope = path`"
            }));
        }
        Ok(())
    })?;
    let column = column.ok_or_else(|| meta.error("expected the column, such as `User::EMAIL`"))?;
    Ok(Some(if unique {
        DatabaseRule::Unique {
            column,
            except,
            case_insensitive,
            scope,
        }
    } else {
        DatabaseRule::Exists { column, scope }
    }))
}

/// The model a column constant belongs to (`User::EMAIL` → `User`).
fn model_of(column: &Path) -> Path {
    let mut model = column.clone();
    model.segments.pop();
    model.segments.pop_punct();
    model
}

/// The expressions that build each field's built-in database rules, each
/// an `Option<DatabaseRule>`.
pub(crate) fn database_rule_builders(fields: &[&Field], krate: &TokenStream2) -> Vec<TokenStream2> {
    fields
        .iter()
        .flat_map(|f| {
            let ident = &f.ident;
            let field = unraw(ident);
            f.rules.iter().filter_map(move |rule| {
                let Rule::Database(rule) = rule else {
                    return None;
                };
                let value = quote!(#krate::Value::from(&self.#ident));
                let scope = |scope: &Option<Path>, column: &Path| {
                    let model = model_of(column);
                    match scope {
                        Some(path) => quote!(::core::option::Option::Some::<#krate::Expr<#model>>(#path(self))),
                        None => quote!(::core::option::Option::None::<#krate::Expr<#model>>),
                    }
                };
                match rule {
                    DatabaseRule::Unique {
                        column,
                        except,
                        case_insensitive,
                        scope: s,
                    } => {
                        let except = match except {
                            Some(other) => quote!(::core::option::Option::Some(#krate::Value::from(&self.#other))),
                            None => quote!(::core::option::Option::None),
                        };
                        let scope = scope(s, column);
                        Some(quote! {
                            #krate::validate::DatabaseRule::unique(#field, #column, #value, #case_insensitive, #except, #scope)
                        })
                    }
                    DatabaseRule::Exists { column, scope: s } => {
                        let scope = scope(s, column);
                        Some(quote! {
                            #krate::validate::DatabaseRule::exists(#field, #column, #value, #scope)
                        })
                    }
                    DatabaseRule::Custom(_) => None,
                }
            })
        })
        .collect()
}

/// The statements that run each field's custom database rule, skipped when
/// `failed` already holds an error for the field.
pub(crate) fn custom_database_steps(fields: &[&Field]) -> Vec<TokenStream2> {
    fields
        .iter()
        .flat_map(|f| {
            let ident = &f.ident;
            let field = unraw(ident);
            f.rules.iter().filter_map(move |rule| match rule {
                Rule::Database(DatabaseRule::Custom(path)) => Some(quote! {
                    if failed.field(#field).next().is_none() {
                        if let ::core::result::Result::Err(message) = #path(&self.#ident, journey.reborrow()).await {
                            found.add(#field, "database", message);
                        }
                    }
                }),
                _ => None,
            })
        })
        .collect()
}
