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
        .filter_map(|f| {
            let ident = &f.ident;
            let field = unraw(ident);
            let v = quote!(#krate::validate);
            let mut checks: Vec<TokenStream2> = f
                .rules
                .iter()
                .zip(&f.messages)
                .filter_map(|(rule, message)| {
                    let (code, check) = rule_check(rule, ident, &field, &v)?;
                    let params = rule_params(rule, krate);
                    let template = template(message.as_ref());
                    Some(quote! {
                        if let ::core::result::Result::Err(message) = #check {
                            errors.add_rule(#field, #code, message, #template, ::std::vec![#(#params),*]);
                        }
                    })
                })
                .collect();
            if f.nested {
                checks.push(quote! {
                    #v::Nested::<<Self as #krate::Input>::Context>::__check_nested(&self.#ident, #field, &mut errors);
                });
            }
            (!checks.is_empty()).then(|| guarded(f, quote!(#(#checks)*)))
        })
        .collect()
}

/// `body`, run only when the field's `when` holds.
pub(crate) fn guarded(field: &Field, body: TokenStream2) -> TokenStream2 {
    match &field.when {
        Some(path) => quote!(if #path(self) { #body }),
        None => body,
    }
}

/// A field rule's code and its check, an expression of type
/// `Result<(), String>`; `None` for a database rule (run later, against a
/// session).
fn rule_check(
    rule: &Rule,
    ident: &Ident,
    field: &str,
    v: &TokenStream2,
) -> Option<(&'static str, TokenStream2)> {
    let opt = |e: &Option<syn::Expr>, cast: TokenStream2| match e {
        Some(e) => quote!(::core::option::Option::Some((#e) as #cast)),
        None => quote!(::core::option::Option::None),
    };
    let required = |condition: TokenStream2| {
        let message = format!("{field} is required");
        quote! {
            if (#condition) && !#v::Presence::is_present(&self.#ident) {
                ::core::result::Result::Err(::std::string::String::from(#message))
            } else {
                ::core::result::Result::<(), ::std::string::String>::Ok(())
            }
        }
    };
    Some(match rule {
        Rule::Length(min, max) => {
            let (min, max) = (opt(min, quote!(usize)), opt(max, quote!(usize)));
            ("length", quote!(#v::length(&self.#ident, #min, #max)))
        }
        Rule::Range(min, max) => {
            let (min, max) = (range_bound(min, v), range_bound(max, v));
            ("range", quote!(#v::bounded(&self.#ident, #min, #max)))
        }
        Rule::Email => ("email", quote!(#v::email(&self.#ident))),
        Rule::NonEmpty => ("non_empty", quote!(#v::non_empty(&self.#ident))),
        Rule::Custom(path) => ("custom", quote!(#path(&self.#ident))),
        Rule::RequiredIf {
            other,
            equals,
            negated,
        } => {
            let condition = if *negated {
                quote!(self.#other != #equals)
            } else {
                quote!(self.#other == #equals)
            };
            ("required", required(condition))
        }
        Rule::RequiredWith(other) => (
            "required",
            required(quote!(#v::Presence::is_present(&self.#other))),
        ),
        Rule::Check(check) => {
            let predicate = check.predicate(v);
            let message = check.message(field);
            (
                check.code(),
                quote! {
                    match #v::AsText::as_text(&self.#ident) {
                        ::core::option::Option::Some(text) if !(#predicate) => {
                            ::core::result::Result::Err(::std::string::String::from(#message))
                        }
                        _ => ::core::result::Result::<(), ::std::string::String>::Ok(()),
                    }
                },
            )
        }
        Rule::Database(_) => return None,
    })
}

/// The statements that run every field's transforms on `self`, in order.
pub(crate) fn transform_steps(fields: &[&Field], krate: &TokenStream2) -> Vec<TokenStream2> {
    fields
        .iter()
        .flat_map(|f| {
            let ident = &f.ident;
            let nested = f.nested.then(
                || quote!(self.#ident = #krate::validate::Nested::<<Self as #krate::Input>::Context>::__transform_nested(self.#ident);),
            );
            f.transforms
                .iter()
                .map(move |transform| match transform {
                    Transform::Builtin(name) => {
                        quote!(self.#ident = #krate::validate::transform::#name(self.#ident);)
                    }
                    Transform::Custom(path) => quote!(self.#ident = #path(self.#ident);),
                })
                .chain(nested)
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
    message: &mut Option<syn::LitStr>,
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
        } else if option.path.is_ident("message") {
            *message = Some(option.value()?.parse()?);
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
pub(crate) fn database_rule_steps(
    fields: &[&Field],
    krate: &TokenStream2,
    with_context: bool,
) -> Vec<TokenStream2> {
    fields
        .iter()
        .filter_map(|f| {
            let ident = &f.ident;
            let field = unraw(ident);
            let mut steps: Vec<TokenStream2> = rule_builders(f, krate, with_context)
                .into_iter()
                .map(|builder| quote!(rules.extend(#builder);))
                .collect();
            if f.nested {
                steps.push(quote! {
                    #krate::validate::Nested::<<Self as #krate::Input>::Context>::__database_nested(&self.#ident, #field, &mut rules, context);
                });
            }
            (!steps.is_empty()).then(|| guarded(f, quote!(#(#steps)*)))
        })
        .collect()
}

/// The expressions that build one field's built-in database rules.
fn rule_builders(f: &Field, krate: &TokenStream2, with_context: bool) -> Vec<TokenStream2> {
    let context = with_context.then(|| quote!(, context));
    let ident = &f.ident;
    let field = unraw(ident);
    std::iter::once(f)
        .flat_map(|f| {
            f.rules.iter().zip(&f.messages).filter_map(|(rule, message)| {
                let Rule::Database(rule) = rule else {
                    return None;
                };
                let template = template(message.as_ref());
                let value = quote!(#krate::Value::from(&self.#ident));
                let scope = |scope: &Option<Path>, column: &Path| {
                    let model = model_of(column);
                    match scope {
                        Some(path) => quote!(::core::option::Option::Some::<#krate::Expr<#model>>(#path(self #context))),
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
                            #krate::validate::DatabaseRule::unique(#field, #column, #value, #case_insensitive, #except, #scope).map(|rule| rule.with_message(#template))
                        })
                    }
                    DatabaseRule::Exists { column, scope: s } => {
                        let scope = scope(s, column);
                        Some(quote! {
                            #krate::validate::DatabaseRule::exists(#field, #column, #value, #scope).map(|rule| rule.with_message(#template))
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
pub(crate) fn custom_database_steps(fields: &[&Field], with_context: bool) -> Vec<TokenStream2> {
    let context = with_context.then(|| quote!(, context));
    let context = &context;
    fields
        .iter()
        .flat_map(|f| {
            let ident = &f.ident;
            let field = unraw(ident);
            f.rules.iter().filter_map(move |rule| match rule {
                Rule::Database(DatabaseRule::Custom(path)) => Some(guarded(
                    f,
                    quote! {
                        if failed.field(#field).next().is_none() {
                            if let ::core::result::Result::Err(message) = #path(&self.#ident, journey.reborrow() #context).await {
                                found.add(#field, "database", message);
                            }
                        }
                    },
                )),
                _ => None,
            })
        })
        .collect()
}

/// A rule's own message as an `Option<&str>` expression.
pub(crate) fn template(message: Option<&syn::LitStr>) -> TokenStream2 {
    match message {
        Some(message) => quote!(::core::option::Option::Some(#message)),
        None => quote!(::core::option::Option::<&str>::None),
    }
}

/// A rule's arguments as `(name, Value)` pairs, for messages and reports.
fn rule_params(rule: &Rule, krate: &TokenStream2) -> Vec<TokenStream2> {
    let bound = |name: &str, e: &Option<syn::Expr>, cast: TokenStream2| {
        e.as_ref()
            .map(|e| quote!((#name, #krate::Value::from((#e) as #cast))))
    };
    match rule {
        Rule::Length(min, max) => [
            bound("min", min, quote!(i64)),
            bound("max", max, quote!(i64)),
        ]
        .into_iter()
        .flatten()
        .collect(),
        Rule::Range(min, max) => [("min", min), ("max", max)]
            .into_iter()
            .filter_map(|(name, bound)| {
                let (e, exclusive) = bound.as_ref()?;
                let name = if *exclusive {
                    format!("exclusive_{name}")
                } else {
                    name.to_owned()
                };
                // An integer literal stays an integer on the wire (`"min": 13`).
                let cast = if is_integer(e) {
                    quote!(i64)
                } else {
                    quote!(f64)
                };
                Some(quote!((#name, #krate::Value::from((#e) as #cast))))
            })
            .collect(),
        Rule::RequiredIf { other, .. } | Rule::RequiredWith(other) => {
            let other = unraw(other);
            vec![quote!(("other", #krate::Value::from(#other)))]
        }
        _ => Vec::new(),
    }
}

/// The statements that add each field's schema to `properties` (and its
/// name to `required` unless it is nullable).
pub(crate) fn schema_steps(fields: &[&Field], krate: &TokenStream2) -> Vec<TokenStream2> {
    fields
        .iter()
        .map(|f| {
            let field = unraw(&f.ident);
            let ty = &f.ty;
            let base = if f.nested {
                quote!(<#ty as #krate::validate::Nested<<Self as #krate::Input>::Context>>::__nested_schema())
            } else {
                quote!((&#krate::validate::Probe::<#ty>::new()).schema())
            };
            let narrowed = f.rules.iter().filter_map(|rule| match rule {
                Rule::Length(min, max) => {
                    let (min, max) = (bound(min, quote!(i64)), bound(max, quote!(i64)));
                    Some(quote!(.length(#min, #max)))
                }
                Rule::Range(min, max) => {
                    let v = quote!(#krate::validate);
                    let (min, max) = (range_bound(min, &v), range_bound(max, &v));
                    Some(quote!(.bounds(#min, #max)))
                }
                Rule::Email => Some(quote!(.format("email"))),
                Rule::NonEmpty => Some(
                    quote!(.length(::core::option::Option::Some(1), ::core::option::Option::None)),
                ),
                _ => None,
            });
            quote! {
                let schema = #base #(#narrowed)*;
                if !schema.is_nullable() {
                    required.push(#field);
                }
                properties.push((#field, schema));
            }
        })
        .collect()
}

/// A bound as an `Option<cast>` expression.
fn bound(e: &Option<syn::Expr>, cast: TokenStream2) -> TokenStream2 {
    match e {
        Some(e) => quote!(::core::option::Option::Some((#e) as #cast)),
        None => quote!(::core::option::Option::None),
    }
}

/// A `range` bound as an `Option<Bound>` expression.
fn range_bound(bound: &Option<(syn::Expr, bool)>, v: &TokenStream2) -> TokenStream2 {
    match bound {
        Some((e, false)) => quote!(::core::option::Option::Some(#v::Bound::inclusive((#e) as f64))),
        Some((e, true)) => quote!(::core::option::Option::Some(#v::Bound::exclusive((#e) as f64))),
        None => quote!(::core::option::Option::None),
    }
}

/// `e` is an integer literal, negated or not.
fn is_integer(e: &syn::Expr) -> bool {
    match e {
        syn::Expr::Lit(lit) => matches!(lit.lit, syn::Lit::Int(_)),
        syn::Expr::Unary(unary) => matches!(unary.op, syn::UnOp::Neg(_)) && is_integer(&unary.expr),
        syn::Expr::Group(group) => is_integer(&group.expr),
        _ => false,
    }
}
