//! The catalogue rules of `#[rok(validate(…))]` — `url`, `uuid`, `ulid`,
//! `ip`, `alpha`, `alpha_numeric`, `ascii`, `one_of`, `not_one_of`,
//! `starts_with`, `ends_with`, `hex_code`, `iban`, `jwt`, `credit_card`,
//! `coordinates` — each a predicate on the field's text from the runtime's
//! `validate::catalogue`, answering the generated APIs' code and message.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{LitInt, LitStr, Token};

/// One catalogue rule, with its arguments.
pub(crate) enum Check {
    Url,
    Uuid(Vec<u8>),
    Ulid,
    Ip(Option<u8>),
    Letters {
        digits: bool,
        spaces: bool,
        dashes: bool,
        underscores: bool,
    },
    Ascii,
    OneOf {
        values: Vec<LitStr>,
        negated: bool,
    },
    StartsWith(LitStr),
    EndsWith(LitStr),
    HexCode,
    Iban,
    Jwt,
    CreditCard(Vec<LitStr>),
    Coordinates,
}

/// `["a", "b"]`.
fn strings(input: syn::parse::ParseStream<'_>) -> syn::Result<Vec<LitStr>> {
    let content;
    syn::bracketed!(content in input);
    Ok(content
        .parse_terminated(|p| p.parse::<LitStr>(), Token![,])?
        .into_iter()
        .collect())
}

/// Parses `rule(…)` options, if any: `message = "…"` and whatever
/// `handle` accepts.
fn options(
    meta: &syn::meta::ParseNestedMeta<'_>,
    message: &mut Option<LitStr>,
    mut handle: impl FnMut(&syn::meta::ParseNestedMeta<'_>) -> syn::Result<bool>,
) -> syn::Result<()> {
    if !meta.input.peek(syn::token::Paren) {
        return Ok(());
    }
    meta.parse_nested_meta(|option| {
        if option.path.is_ident("message") {
            *message = Some(option.value()?.parse()?);
            Ok(())
        } else if handle(&option)? {
            Ok(())
        } else {
            Err(option.error("unknown option for this rule"))
        }
    })
}

/// The catalogue rule `meta` names, or `None` for another rule.
pub(crate) fn parse_check(
    meta: &syn::meta::ParseNestedMeta<'_>,
    message: &mut Option<LitStr>,
) -> syn::Result<Option<Check>> {
    let name = match meta.path.get_ident() {
        Some(name) => name.to_string(),
        None => return Ok(None),
    };
    Ok(match name.as_str() {
        "url" | "ulid" | "ascii" | "hex_code" | "iban" | "jwt" | "coordinates" => {
            options(meta, message, |_| Ok(false))?;
            Some(match name.as_str() {
                "url" => Check::Url,
                "ulid" => Check::Ulid,
                "ascii" => Check::Ascii,
                "hex_code" => Check::HexCode,
                "iban" => Check::Iban,
                "jwt" => Check::Jwt,
                _ => Check::Coordinates,
            })
        }
        "uuid" => {
            let mut versions = Vec::new();
            options(meta, message, |option| {
                if option.path.is_ident("versions") {
                    let input = option.value()?;
                    let content;
                    syn::bracketed!(content in input);
                    for version in content.parse_terminated(|p| p.parse::<LitInt>(), Token![,])? {
                        versions.push(version.base10_parse()?);
                    }
                    Ok(true)
                } else {
                    Ok(false)
                }
            })?;
            Some(Check::Uuid(versions))
        }
        "ip" => {
            let mut version = None;
            options(meta, message, |option| {
                if option.path.is_ident("version") {
                    version = Some(option.value()?.parse::<LitInt>()?.base10_parse()?);
                    Ok(true)
                } else {
                    Ok(false)
                }
            })?;
            Some(Check::Ip(version))
        }
        "alpha" | "alpha_numeric" => {
            let (mut spaces, mut dashes, mut underscores) = (false, false, false);
            options(meta, message, |option| {
                let flag = if option.path.is_ident("spaces") {
                    &mut spaces
                } else if option.path.is_ident("dashes") {
                    &mut dashes
                } else if option.path.is_ident("underscores") {
                    &mut underscores
                } else {
                    return Ok(false);
                };
                *flag = true;
                Ok(true)
            })?;
            Some(Check::Letters {
                digits: name == "alpha_numeric",
                spaces,
                dashes,
                underscores,
            })
        }
        "one_of" | "not_one_of" => {
            let mut values = Vec::new();
            options(meta, message, |option| {
                if option.path.is_ident("values") {
                    values = strings(option.value()?)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            })?;
            if values.is_empty() {
                return Err(meta.error("expected `values = [\"…\", …]`"));
            }
            Some(Check::OneOf {
                values,
                negated: name == "not_one_of",
            })
        }
        "starts_with" | "ends_with" => {
            let text: LitStr = meta.value()?.parse()?;
            Some(if name == "starts_with" {
                Check::StartsWith(text)
            } else {
                Check::EndsWith(text)
            })
        }
        "credit_card" => {
            let mut providers = Vec::new();
            options(meta, message, |option| {
                if option.path.is_ident("providers") {
                    providers = strings(option.value()?)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            })?;
            Some(Check::CreditCard(providers))
        }
        _ => None,
    })
}

impl Check {
    /// The wire code: the generated APIs' rule name.
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Check::Url => "url",
            Check::Uuid(_) => "uuid",
            Check::Ulid => "ulid",
            Check::Ip(_) => "ipAddress",
            Check::Letters { digits: false, .. } => "alpha",
            Check::Letters { digits: true, .. } => "alphaNumeric",
            Check::Ascii => "ascii",
            Check::OneOf { negated: false, .. } => "in",
            Check::OneOf { negated: true, .. } => "notIn",
            Check::StartsWith(_) => "startsWith",
            Check::EndsWith(_) => "endsWith",
            Check::HexCode => "hexCode",
            Check::Iban => "iban",
            Check::Jwt => "jwt",
            Check::CreditCard(_) => "creditCard",
            Check::Coordinates => "coordinates",
        }
    }

    /// The generated APIs' English message, naming `field`.
    pub(crate) fn message(&self, field: &str) -> String {
        let what = match self {
            Check::Url => "be a URL".to_owned(),
            Check::Uuid(_) => "be a UUID".to_owned(),
            Check::Ulid => "be a ULID".to_owned(),
            Check::Ip(None) => "be an IP address".to_owned(),
            Check::Ip(Some(version)) => format!("be an IPv{version} address"),
            Check::Letters {
                digits,
                spaces,
                dashes,
                underscores,
            } => {
                let mut kinds = vec!["letters"];
                if *digits {
                    kinds.push("digits");
                }
                for (on, kind) in [
                    (*spaces, "spaces"),
                    (*dashes, "dashes"),
                    (*underscores, "underscores"),
                ] {
                    if on {
                        kinds.push(kind);
                    }
                }
                let listed = match kinds.split_last() {
                    Some((last, rest)) if !rest.is_empty() => {
                        format!("{} and {last}", rest.join(", "))
                    }
                    _ => kinds.concat(),
                };
                format!("contain only {listed}")
            }
            Check::Ascii => "contain only ASCII characters".to_owned(),
            Check::OneOf { values, negated } => {
                let listed: Vec<String> = values.iter().map(LitStr::value).collect();
                let not = if *negated { "not " } else { "" };
                format!("{not}be one of {}", listed.join(", "))
            }
            Check::StartsWith(text) => format!("start with {}", text.value()),
            Check::EndsWith(text) => format!("end with {}", text.value()),
            Check::HexCode => "be a hex color code".to_owned(),
            Check::Iban => "be an IBAN".to_owned(),
            Check::Jwt => "be a JWT".to_owned(),
            Check::CreditCard(_) => "be a credit card number".to_owned(),
            Check::Coordinates => "be latitude and longitude coordinates".to_owned(),
        };
        format!("{field} must {what}")
    }

    /// The predicate on `text: &str`, true when the value passes; `validate`
    /// is the runtime's `validate` module path.
    pub(crate) fn predicate(&self, validate: &TokenStream2) -> TokenStream2 {
        let c = quote!(#validate::catalogue);
        match self {
            Check::Url => quote!(#c::is_url(text)),
            Check::Uuid(versions) => quote!(#c::is_uuid(text, &[#(#versions),*])),
            Check::Ulid => quote!(#c::is_ulid(text)),
            Check::Ip(version) => match version {
                Some(v) => quote!(#c::is_ip(text, ::core::option::Option::Some(#v))),
                None => quote!(#c::is_ip(text, ::core::option::Option::None)),
            },
            Check::Letters {
                digits,
                spaces,
                dashes,
                underscores,
            } => quote!(#c::letters(text, #digits, #spaces, #dashes, #underscores)),
            Check::Ascii => quote!(#c::is_ascii(text)),
            Check::OneOf { values, negated } => {
                let contains = quote!([#(#values),*].contains(&text));
                if *negated {
                    quote!(!#contains)
                } else {
                    contains
                }
            }
            Check::StartsWith(prefix) => quote!(text.starts_with(#prefix)),
            Check::EndsWith(suffix) => quote!(text.ends_with(#suffix)),
            Check::HexCode => quote!(#c::is_hex_code(text)),
            Check::Iban => quote!(#c::is_iban(text)),
            Check::Jwt => quote!(#c::is_jwt(text)),
            Check::CreditCard(providers) => quote!(#c::is_credit_card(text, &[#(#providers),*])),
            Check::Coordinates => quote!(#c::is_coordinates(text)),
        }
    }
}
