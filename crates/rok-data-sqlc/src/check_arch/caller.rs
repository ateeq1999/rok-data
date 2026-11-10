//! The caller-first rule: a handler takes its caller extractor before its
//! body extractor, so a caller without permission gets the 403 before the
//! body's 422 tells them its rules.
//!
//! The caller extractors are `AuthUser`, `AdminUser`, and every struct the
//! App marks with a `/// rok:caller` doc line; the body extractors are
//! `ValidatedJson`, `Json` and `Form`. Test code is exempt.

use syn::{Attribute, FnArg, Item, Meta, Type};

/// The caller extractors every App has.
const CALLERS: &[&str] = &["AuthUser", "AdminUser"];

/// The extractors that read the request body.
const BODIES: &[&str] = &["ValidatedJson", "Json", "Form"];

/// The marker that makes a struct a caller extractor.
const MARKER: &str = "rok:caller";

/// The structs `file` marks as caller extractors.
pub(super) fn marked(file: &syn::File) -> Vec<String> {
    let mut found = Vec::new();
    collect_marked(&file.items, &mut found);
    found
}

fn collect_marked(items: &[Item], found: &mut Vec<String>) {
    for item in items {
        match item {
            Item::Struct(item) if is_marked(&item.attrs) => found.push(item.ident.to_string()),
            Item::Mod(module) if !is_test(&module.attrs) => {
                if let Some((_, items)) = &module.content {
                    collect_marked(items, found);
                }
            }
            _ => {}
        }
    }
}

fn is_marked(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| match &attr.meta {
        Meta::NameValue(pair) if pair.path.is_ident("doc") => {
            matches!(&pair.value, syn::Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Str(text) if text.value().trim() == MARKER))
        }
        _ => false,
    })
}

/// `#[cfg(test)]` or `#[test]`.
fn is_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("test")
            || (attr.path().is_ident("cfg")
                && attr
                    .parse_args::<syn::Ident>()
                    .is_ok_and(|ident| ident == "test"))
    })
}

/// Each handler of `file` that takes a body before a caller: its line and
/// the message. `marked` names the App's own caller extractors.
pub(super) fn violations(file: &syn::File, marked: &[String]) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    check_items(&file.items, marked, &mut found);
    found
}

fn check_items(items: &[Item], marked: &[String], found: &mut Vec<(usize, String)>) {
    for item in items {
        match item {
            Item::Fn(function) if !is_test(&function.attrs) => {
                let names: Vec<Option<String>> = function
                    .sig
                    .inputs
                    .iter()
                    .map(|input| match input {
                        FnArg::Typed(typed) => last_segment(&typed.ty),
                        FnArg::Receiver(_) => None,
                    })
                    .collect();
                let is_caller = |name: &str| {
                    CALLERS.contains(&name) || marked.iter().any(|caller| caller == name)
                };
                let body = names
                    .iter()
                    .position(|name| name.as_deref().is_some_and(|name| BODIES.contains(&name)));
                let caller = names
                    .iter()
                    .position(|name| name.as_deref().is_some_and(is_caller));
                if let (Some(body), Some(caller)) = (body, caller)
                    && body < caller
                {
                    let (body, caller) = (
                        names[body].clone().unwrap_or_default(),
                        names[caller].clone().unwrap_or_default(),
                    );
                    found.push((
                        function.sig.ident.span().start().line,
                        format!(
                            "`{}` takes its body (`{body}`) before its caller (`{caller}`), so a \
                             caller without permission sees the body's 422 before the 403; take \
                             the caller first",
                            function.sig.ident
                        ),
                    ));
                }
            }
            Item::Mod(module) if !is_test(&module.attrs) => {
                if let Some((_, items)) = &module.content {
                    check_items(items, marked, found);
                }
            }
            _ => {}
        }
    }
}

/// The last path segment of `ty`, behind references: `ValidatedJson` for
/// `ValidatedJson<Dto>`, `AuthUser` for `&AuthUser`.
fn last_segment(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Type::Reference(reference) => last_segment(&reference.elem),
        Type::Paren(inner) => last_segment(&inner.elem),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(source: &str, marked: &[&str]) -> Vec<usize> {
        let file = syn::parse_file(source).unwrap_or_else(|err| panic!("{err}"));
        let marked: Vec<String> = marked.iter().map(|name| (*name).to_owned()).collect();
        violations(&file, &marked)
            .into_iter()
            .map(|(line, _)| line)
            .collect()
    }

    #[test]
    fn a_body_before_a_caller_is_named_and_caller_first_passes() {
        let source = "\
pub async fn caller_first(caller: AuthUser, body: ValidatedJson<Dto>) {}
pub async fn body_first(body: ValidatedJson<Dto>, caller: AuthUser) {}
pub async fn admin(body: web::Json<Dto>, admin: &AdminUser) {}
pub async fn no_caller(body: ValidatedJson<Dto>) {}
#[cfg(test)]
mod tests { fn exempt(body: Json<Dto>, caller: AuthUser) {} }
";
        assert_eq!(lines(source, &[]), [2, 3]);
        let file = syn::parse_file(source).unwrap_or_else(|err| panic!("{err}"));
        let message = &violations(&file, &[])[0].1;
        assert!(
            message.contains(
                "`body_first` takes its body (`ValidatedJson`) before its caller (`AuthUser`)"
            ),
            "{message}"
        );
    }

    #[test]
    fn the_app_marks_its_own_caller_extractors() {
        let marker = "\
/// A member of the organization in the path.
///
/// rok:caller
pub struct Member { id: i64 }
pub struct NotACaller;
";
        let file = syn::parse_file(marker).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(marked(&file), ["Member"]);
        let handler = "pub async fn join(body: Json<Dto>, member: Member) {}";
        assert_eq!(lines(handler, &[]), Vec::<usize>::new());
        assert_eq!(lines(handler, &["Member"]), [1]);
    }
}
